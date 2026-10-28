use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use ashpd::desktop::screencast::{CursorMode, Screencast, SelectSourcesOptions, SourceType};
use ashpd::desktop::{PersistMode, Session};
use futures_util::StreamExt;
use gst::prelude::*;
use gstreamer as gst;

use super::compositor::{Compositor, detect};
use super::windowing::focused_output_name;
use crate::platform::Region;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortalSourceType {
    Window,
    Monitor,
}

mod frames;

pub fn spawn_fullscreen(with_audio: bool, output_path: &Path) -> Result<Child> {
    match detect() {
        Compositor::Wlroots => {
            let mut command = Command::new("wf-recorder");
            if let Ok(output_name) = focused_output_name() {
                command.args(["-o", &output_name]);
            }
            spawn(command, with_audio, output_path)
        }
        Compositor::Kde | Compositor::Gnome => {
            spawn_portal(PortalSourceType::Monitor, with_audio, output_path)
        }
    }
}

pub fn spawn_region(region: &Region, with_audio: bool, output_path: &Path) -> Result<Child> {
    let mut command = Command::new("wf-recorder");
    command.args(["-g", &region.to_slurp_geometry()]);
    spawn(command, with_audio, output_path)
}

pub fn spawn_window(with_audio: bool, output_path: &Path) -> Result<Child> {
    spawn_portal(PortalSourceType::Window, with_audio, output_path)
}

fn spawn_portal(
    source_type: PortalSourceType,
    with_audio: bool,
    output_path: &Path,
) -> Result<Child> {
    let executable = std::env::current_exe().context("无法定位 ncaptura 可执行文件")?;
    let mut command = Command::new(executable);
    match source_type {
        PortalSourceType::Window => command.arg("__record-window"),
        PortalSourceType::Monitor => command.arg("__record-monitor"),
    };
    command.arg(output_path);
    if with_audio {
        command.arg("--audio");
    }
    let label = match source_type {
        PortalSourceType::Window => "窗口",
        PortalSourceType::Monitor => "全屏",
    };
    command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .with_context(|| format!("无法启动{label}录屏进程"))
}

pub fn run_window_recording(output_path: PathBuf, with_audio: bool) -> Result<()> {
    run_portal_recording(PortalSourceType::Window, output_path, with_audio)
}

pub fn run_monitor_recording(output_path: PathBuf, with_audio: bool) -> Result<()> {
    run_portal_recording(PortalSourceType::Monitor, output_path, with_audio)
}

pub fn run_portal_recording(
    source_type: PortalSourceType,
    output_path: PathBuf,
    with_audio: bool,
) -> Result<()> {
    let label = match source_type {
        PortalSourceType::Window => "窗口",
        PortalSourceType::Monitor => "全屏",
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .with_context(|| format!("无法初始化{label}录屏运行时"))?;
    runtime.block_on(async move {
        run_portal_recording_async(source_type, output_path, with_audio).await
    })
}

async fn run_portal_recording_async(
    source_type: PortalSourceType,
    output_path: PathBuf,
    with_audio: bool,
) -> Result<()> {
    let label = match source_type {
        PortalSourceType::Window => "窗口",
        PortalSourceType::Monitor => "全屏",
    };
    gst::init().context("无法初始化 GStreamer")?;
    let interrupted = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let _interrupt = signal_hook::flag::register(signal_hook::consts::SIGINT, interrupted.clone())
        .context("无法注册录屏停止信号")?;
    let _terminate = signal_hook::flag::register(signal_hook::consts::SIGTERM, interrupted.clone())
        .context("无法注册录屏退出信号")?;
    let stop = async move {
        while !interrupted.load(std::sync::atomic::Ordering::Relaxed) {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    };
    tokio::pin!(stop);
    let source = tokio::select! {
        result = PortalCaptureSource::request(source_type) => result?,
        () = &mut stop => return Ok(()),
    };
    let result = async {
        let mut closed = source
            .session
            .receive_closed()
            .await
            .with_context(|| format!("无法监测{label}录屏会话"))?;
        let pipeline = WindowRecordingPipeline::from_remote(
            source.node_id,
            source
                .remote
                .try_clone()
                .context("无法复制 PipeWire 录屏连接")?,
            with_audio,
            &output_path,
        )?;
        pipeline
            .run(async {
                tokio::select! {
                    () = &mut stop => {},
                    _ = closed.next() => {},
                }
            })
            .await
    }
    .await;
    let _ = source.session.close().await;
    result
}

struct PortalCaptureSource {
    session: Session<Screencast>,
    node_id: u32,
    remote: OwnedFd,
}

impl PortalCaptureSource {
    async fn request(source_type: PortalSourceType) -> Result<Self> {
        let ashpd_source_type = match source_type {
            PortalSourceType::Window => SourceType::Window,
            PortalSourceType::Monitor => SourceType::Monitor,
        };
        let label = match source_type {
            PortalSourceType::Window => "窗口",
            PortalSourceType::Monitor => "全屏",
        };

        let portal = Screencast::new()
            .await
            .context("无法连接 ScreenCast portal")?;
        if !portal
            .available_source_types()
            .await
            .context("无法查询录屏来源类型")?
            .contains(ashpd_source_type)
        {
            bail!("当前 ScreenCast portal 不支持{label}捕获");
        }
        let cursor_modes = portal
            .available_cursor_modes()
            .await
            .context("无法查询录屏光标模式")?;
        let cursor_mode = if cursor_modes.contains(CursorMode::Embedded) {
            CursorMode::Embedded
        } else if cursor_modes.contains(CursorMode::Hidden) {
            CursorMode::Hidden
        } else {
            bail!("当前 ScreenCast portal 不支持可用的录屏光标模式");
        };
        let session = portal
            .create_session(Default::default())
            .await
            .with_context(|| format!("无法创建{label}录屏会话"))?;
        portal
            .select_sources(
                &session,
                SelectSourcesOptions::default()
                    .set_cursor_mode(cursor_mode)
                    .set_sources(ashpd::enumflags2::BitFlags::from_flag(ashpd_source_type))
                    .set_multiple(false)
                    .set_persist_mode(PersistMode::DoNot),
            )
            .await
            .with_context(|| format!("无法请求{label}录屏来源"))?
            .response()
            .with_context(|| format!("{label}录屏来源请求被拒绝"))?;
        let streams = portal
            .start(&session, None, Default::default())
            .await
            .with_context(|| format!("无法启动{label}录屏来源"))?
            .response()
            .with_context(|| format!("{label}录屏来源选择已取消或失败"))?;
        let stream = streams
            .streams()
            .first()
            .filter(|_| streams.streams().len() == 1)
            .ok_or_else(|| anyhow!("{label}录屏必须返回一个视频流"))?;
        if stream
            .source_type()
            .is_some_and(|kind| kind != ashpd_source_type)
        {
            bail!("ScreenCast 返回的来源类型不匹配");
        }
        let remote = portal
            .open_pipe_wire_remote(&session, Default::default())
            .await
            .context("无法打开 PipeWire 录屏连接")?;
        Ok(Self {
            session,
            node_id: stream.pipe_wire_node_id(),
            remote,
        })
    }
}

struct WindowRecordingPipeline {
    pipeline: gst::Pipeline,
    pump: Option<frames::FramePump>,
}

impl WindowRecordingPipeline {
    fn from_remote(
        node_id: u32,
        remote: OwnedFd,
        with_audio: bool,
        output_path: &Path,
    ) -> Result<Self> {
        let frame_source = frames::FrameSource::connect(remote, node_id)?;
        let source = frames::build_appsrc(frame_source.format())?;
        let audio_source = if with_audio {
            let mut builder = gst::ElementFactory::make("pulsesrc");
            let device = default_system_mix_audio_device().context(
                "无法读取系统音频输出；请安装 pactl 并确认 PulseAudio / PipeWire Pulse 服务可用",
            )?;
            builder = builder.property("device", device);
            Some(builder.build().context("无法创建音频源")?)
        } else {
            None
        };
        let mut recording = Self::new(source.clone(), audio_source, output_path)?;
        recording.pump = Some(frame_source.pump_into(source));
        Ok(recording)
    }

    fn new(
        video_source: gst::Element,
        audio_source: Option<gst::Element>,
        output_path: &Path,
    ) -> Result<Self> {
        let video_chain = vec![video_source.clone()];
        let mut video_chain = video_chain;
        let convert = gst::ElementFactory::make("videoconvert")
            .build()
            .context("无法创建视频转换器")?;
        let scale = gst::ElementFactory::make("videoscale")
            .build()
            .context("无法创建视频缩放器")?;
        let canvas = gst::ElementFactory::make("capsfilter")
            .build()
            .context("无法创建录屏画布")?;
        let canvas_handle = canvas.clone();
        let initialized = AtomicBool::new(false);
        video_source
            .static_pad("src")
            .context("窗口视频源缺少输出接口")?
            .add_probe(gst::PadProbeType::EVENT_DOWNSTREAM, move |_, info| {
                if let Some(gst::EventView::Caps(event)) = info.event().map(|event| event.view())
                    && !initialized.load(Ordering::Relaxed)
                    && let Ok(caps) = window_canvas_caps(event.caps())
                {
                    canvas_handle.set_property("caps", caps);
                    initialized.store(true, Ordering::Relaxed);
                }
                gst::PadProbeReturn::Ok
            });
        let first_frame = AtomicBool::new(false);
        video_source
            .static_pad("src")
            .context("窗口视频源缺少输出接口")?
            .add_probe(gst::PadProbeType::BUFFER, move |_, _| {
                if !first_frame.swap(true, Ordering::Relaxed) {
                    println!("READY");
                }
                gst::PadProbeReturn::Ok
            });
        let encoder = gst::ElementFactory::make("x264enc")
            .property_from_str("tune", "zerolatency")
            .property_from_str("speed-preset", "ultrafast")
            .property("key-int-max", 60u32)
            .build()
            .context("无法创建 H.264 编码器")?;
        let video_queue = gst::ElementFactory::make("queue")
            .build()
            .context("无法创建视频队列")?;
        let muxer = gst::ElementFactory::make("matroskamux")
            .build()
            .context("无法创建 Matroska 封装器")?;
        let sink = gst::ElementFactory::make("filesink")
            .property("location", output_path.to_string_lossy().as_ref())
            .build()
            .context("无法创建录屏文件输出")?;

        video_chain.extend([
            convert,
            scale,
            canvas,
            encoder,
            video_queue,
            muxer.clone(),
            sink,
        ]);
        let pipeline = gst::Pipeline::default();
        let video_refs: Vec<&gst::Element> = video_chain.iter().collect();
        pipeline
            .add_many(video_refs.as_slice())
            .context("无法组装窗口录屏视频管线")?;
        gst::Element::link_many(video_refs.as_slice()).context("无法连接窗口录屏视频管线")?;

        if let Some(audio_source) = audio_source {
            let audio_convert = gst::ElementFactory::make("audioconvert")
                .build()
                .context("无法创建音频转换器")?;
            let audio_resample = gst::ElementFactory::make("audioresample")
                .build()
                .context("无法创建音频重采样器")?;
            let audio_encoder = gst::ElementFactory::make("vorbisenc")
                .build()
                .context("无法创建音频编码器")?;
            let audio_queue = gst::ElementFactory::make("queue")
                .build()
                .context("无法创建音频队列")?;
            pipeline
                .add_many([
                    &audio_source,
                    &audio_convert,
                    &audio_resample,
                    &audio_encoder,
                    &audio_queue,
                ])
                .context("无法组装窗口录屏音频管线")?;
            gst::Element::link_many([
                &audio_source,
                &audio_convert,
                &audio_resample,
                &audio_encoder,
                &audio_queue,
            ])
            .context("无法连接窗口录屏音频管线")?;
            audio_queue.link(&muxer).context("无法连接音频封装器")?;
        }

        Ok(Self {
            pipeline,
            pump: None,
        })
    }

    async fn run(&self, stop: impl std::future::Future<Output = ()>) -> Result<()> {
        let result = self.run_until_eos(stop).await;
        self.pipeline
            .set_state(gst::State::Null)
            .context("无法关闭窗口录屏管线")?;
        result
    }

    async fn run_until_eos(&self, stop: impl std::future::Future<Output = ()>) -> Result<()> {
        self.start_pipeline()?;
        let bus = self.pipeline.bus().context("窗口录屏管线没有消息总线")?;
        let mut stop_requested: Option<Instant> = None;
        let started = Instant::now();
        let mut playing = false;
        tokio::pin!(stop);
        loop {
            if stop_requested.is_some_and(|started| started.elapsed() > Duration::from_secs(5)) {
                return Err(anyhow!("等待窗口录屏文件收尾超时"));
            }
            if !playing && stop_requested.is_none() && started.elapsed() > Duration::from_secs(10) {
                bail!("等待窗口视频流超时");
            }
            while let Some(message) = bus.pop() {
                match message.view() {
                    gst::MessageView::Eos(..) => return Ok(()),
                    gst::MessageView::Error(error) => {
                        return Err(anyhow!("窗口录屏管线失败: {}", error.error()));
                    }
                    gst::MessageView::StateChanged(state)
                        if message.src() == Some(self.pipeline.upcast_ref()) =>
                    {
                        playing = state.current() == gst::State::Playing;
                    }
                    _ => {}
                }
            }
            tokio::select! {
                () = &mut stop, if stop_requested.is_none() => {
                    if !playing {
                        bail!("窗口录屏在收到视频前已停止");
                    }
                    if !self.pipeline.send_event(gst::event::Eos::new()) {
                        bail!("无法结束窗口录屏视频流");
                    }
                    stop_requested = Some(Instant::now());
                },
                () = tokio::time::sleep(Duration::from_millis(20)) => {},
            }
        }
    }
}

impl WindowRecordingPipeline {
    fn start_pipeline(&self) -> Result<()> {
        let change = self.pipeline.set_state(gst::State::Playing);
        if change.is_ok() {
            return Ok(());
        }

        let failure = self
            .pipeline
            .bus()
            .and_then(|bus| {
                bus.timed_pop_filtered(gst::ClockTime::from_seconds(1), &[gst::MessageType::Error])
            })
            .map(|message| match message.view() {
                gst::MessageView::Error(error) => {
                    let source = error
                        .src()
                        .map(|source| source.path_string().to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    let debug = error
                        .debug()
                        .map(|debug| format!(" ({debug})"))
                        .unwrap_or_default();
                    format!("{source}: {}{debug}", error.error())
                }
                _ => "未知状态切换错误".to_string(),
            })
            .unwrap_or_else(|| "没有收到元素错误".to_string());
        bail!("无法启动窗口录屏管线: {failure}");
    }
}

fn window_canvas_caps(input: &gst::CapsRef) -> Result<gst::Caps> {
    let structure = input.structure(0).context("窗口视频没有格式信息")?;
    let dimension = |name| -> Result<i32> {
        let value = structure.get::<i32>(name)?;
        if value <= 0 {
            bail!("窗口视频尺寸必须为正数");
        }
        value.checked_add(value % 2).context("窗口视频尺寸过大")
    };
    Ok(gst::Caps::builder("video/x-raw")
        .field("format", "I420")
        .field("width", dimension("width")?)
        .field("height", dimension("height")?)
        .field("pixel-aspect-ratio", gst::Fraction::new(1, 1))
        .build())
}

impl Drop for WindowRecordingPipeline {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}

fn spawn(mut command: Command, with_audio: bool, output_path: &Path) -> Result<Child> {
    if with_audio {
        if let Some(audio_device) = default_system_mix_audio_device() {
            command.arg(format!("--audio={audio_device}"));
        } else {
            command.arg("--audio");
        }
    }

    command.arg("-f").arg(output_path);
    command
        .spawn()
        .context("无法启动 wf-recorder，请确认已安装并在 PATH 中")
}

fn default_system_mix_audio_device() -> Option<String> {
    let output = Command::new("pactl")
        .arg("get-default-sink")
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let sink_name = String::from_utf8(output.stdout).ok()?;
    let sink_name = sink_name.trim();

    if sink_name.is_empty() {
        return None;
    }

    Some(format!("{sink_name}.monitor"))
}

#[cfg(test)]
mod tests;
