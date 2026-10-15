use std::io::Cursor;
use std::os::fd::{AsRawFd, OwnedFd};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, sync_channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use gst::prelude::*;
use gstreamer as gst;
use libspa::buffer::{Data, DataType};
use libspa::param::ParamType;
use libspa::param::format::{FormatProperties, MediaSubtype, MediaType};
use libspa::param::video::{VideoFormat, VideoInfoRaw};
use libspa::pod::serialize::PodSerializer;
use libspa::pod::{ChoiceValue, Object, Pod, Property, PropertyFlags, Value};
use libspa::sys as spa_sys;
use libspa::utils::{
    Choice, ChoiceEnum, ChoiceFlags, Direction, Fraction, Id, Rectangle, SpaTypes,
};
use pipewire::properties::properties;
use pipewire::stream::{StreamFlags, StreamState};

const LINEAR_MODIFIER: i64 = 0;
const DMA_BUF_IOCTL_SYNC: libc::c_ulong = 0x4008_6200;
const DMA_BUF_SYNC_READ: u64 = 1;
const DMA_BUF_SYNC_START: u64 = 0;
const DMA_BUF_SYNC_END: u64 = 1 << 2;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const FRAME_QUEUE: usize = 4;
const FRAME_RATE: i32 = 30;
const FRAME_INTERVAL: Duration = Duration::from_millis(1000 / FRAME_RATE as u64);

#[repr(C)]
struct DmaBufSync {
    flags: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameFormat {
    pub name: &'static str,
    pub width: u32,
    pub height: u32,
}

struct Frame {
    format: FrameFormat,
    data: Vec<u8>,
}

enum Event {
    Format(FrameFormat),
    Frame(Frame),
    Failed(String),
}

pub struct FrameSource {
    events: Receiver<Event>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    format: FrameFormat,
}

impl FrameSource {
    pub fn connect(remote: OwnedFd, node_id: u32) -> Result<Self> {
        let (sender, events) = sync_channel(FRAME_QUEUE);
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let worker = std::thread::Builder::new()
            .name("ncaptura-pipewire".into())
            .spawn(move || {
                if let Err(error) = run_stream(remote, node_id, &sender, &worker_stop) {
                    let _ = sender.send(Event::Failed(format!("{error:#}")));
                }
            })
            .context("无法启动 PipeWire 取帧线程")?;
        let mut source = Self {
            events,
            stop,
            worker: Some(worker),
            format: FrameFormat {
                name: "BGRx",
                width: 0,
                height: 0,
            },
        };
        let deadline = Instant::now() + CONNECT_TIMEOUT;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match source.events.recv_timeout(remaining) {
                Ok(Event::Format(format)) => {
                    source.format = format;
                    return Ok(source);
                }
                Ok(Event::Failed(message)) => bail!("窗口视频流协商失败: {message}"),
                Ok(Event::Frame(_)) => {}
                Err(_) => bail!("等待窗口视频流格式超时"),
            }
        }
    }

    pub fn format(&self) -> FrameFormat {
        self.format
    }

    pub fn pump_into(mut self, appsrc: gst::Element) -> FramePump {
        let events = std::mem::replace(&mut self.events, sync_channel(1).1);
        let pusher = std::thread::Builder::new()
            .name("ncaptura-frames".into())
            .spawn(move || push_frames(&events, &appsrc))
            .ok();
        FramePump {
            source: self,
            pusher,
        }
    }
}

impl Drop for FrameSource {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub struct FramePump {
    source: FrameSource,
    pusher: Option<JoinHandle<()>>,
}

impl Drop for FramePump {
    fn drop(&mut self) {
        self.source.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.source.worker.take() {
            let _ = worker.join();
        }
        if let Some(pusher) = self.pusher.take() {
            let _ = pusher.join();
        }
    }
}

struct SharedFrame(Arc<Vec<u8>>);

impl AsRef<[u8]> for SharedFrame {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

fn push_frames(events: &Receiver<Event>, appsrc: &gst::Element) {
    let mut latest: Option<(FrameFormat, Arc<Vec<u8>>)> = None;
    let mut current: Option<FrameFormat> = None;
    let mut next = Instant::now();
    loop {
        let wait = next.saturating_duration_since(Instant::now());
        match events.recv_timeout(wait) {
            Ok(Event::Frame(frame)) => latest = Some((frame.format, Arc::new(frame.data))),
            Ok(Event::Format(_)) => {}
            Ok(Event::Failed(message)) => {
                let _ = appsrc.post_message(
                    gst::message::Error::builder(gst::StreamError::Failed, &message).build(),
                );
                return;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        let now = Instant::now();
        if now < next {
            continue;
        }
        next = (next + FRAME_INTERVAL).max(now);
        let Some((format, data)) = &latest else {
            continue;
        };
        if current != Some(*format) {
            appsrc.set_property("caps", frame_caps(*format));
            current = Some(*format);
        }
        let buffer = gst::Buffer::from_slice(SharedFrame(data.clone()));
        let flow = appsrc.emit_by_name::<gst::FlowReturn>("push-buffer", &[&buffer]);
        if matches!(flow, gst::FlowReturn::Flushing | gst::FlowReturn::Eos) {
            return;
        }
    }
}

pub fn frame_caps(format: FrameFormat) -> gst::Caps {
    gst::Caps::builder("video/x-raw")
        .field("format", format.name)
        .field("width", format.width as i32)
        .field("height", format.height as i32)
        .field("framerate", gst::Fraction::new(FRAME_RATE, 1))
        .build()
}

pub fn build_appsrc(format: FrameFormat) -> Result<gst::Element> {
    gst::ElementFactory::make("appsrc")
        .property("caps", frame_caps(format))
        .property("is-live", true)
        .property("do-timestamp", true)
        .property("max-bytes", 64u64 * 1024 * 1024)
        .property_from_str("format", "time")
        .property_from_str("leaky-type", "downstream")
        .build()
        .context("无法创建窗口画面输入源")
}

struct FrameStreamState {
    sender: SyncSender<Event>,
    format: Option<(&'static str, u32, u32)>,
}

fn run_stream(
    remote: OwnedFd,
    node_id: u32,
    sender: &SyncSender<Event>,
    stop: &AtomicBool,
) -> Result<()> {
    pipewire::init();
    let mainloop =
        pipewire::main_loop::MainLoopRc::new(None).context("无法创建 PipeWire 主循环")?;
    let context =
        pipewire::context::ContextRc::new(&mainloop, None).context("无法创建 PipeWire 上下文")?;
    let core = context
        .connect_fd_rc(remote, None)
        .context("无法连接 PipeWire 录屏会话")?;
    let stream = pipewire::stream::StreamRc::new(
        core,
        "ncaptura-window",
        properties! {
            *pipewire::keys::MEDIA_TYPE => "Video",
            *pipewire::keys::MEDIA_CATEGORY => "Capture",
            *pipewire::keys::MEDIA_ROLE => "Screen",
        },
    )
    .context("无法创建窗口视频流")?;
    let failure = Arc::new(std::sync::Mutex::new(None::<String>));
    let failure_cb = failure.clone();
    let _listener = stream
        .add_local_listener_with_user_data(FrameStreamState {
            sender: sender.clone(),
            format: None,
        })
        .state_changed(move |_, _, _, state| {
            if let StreamState::Error(message) = state {
                *failure_cb.lock().unwrap() = Some(message);
            }
        })
        .param_changed(|stream, state, id, param| {
            let Some(param) = param else {
                return;
            };
            if id != ParamType::Format.as_raw() {
                return;
            }
            let mut info = VideoInfoRaw::new();
            if info.parse(param).is_err() {
                return;
            }
            let name = match info.format() {
                VideoFormat::BGRx => "BGRx",
                VideoFormat::BGRA => "BGRA",
                _ => return,
            };
            let size = info.size();
            state.format = Some((name, size.width, size.height));
            let _ = state.sender.try_send(Event::Format(FrameFormat {
                name,
                width: size.width,
                height: size.height,
            }));
            if let Ok(bytes) = buffers_param()
                && let Some(pod) = Pod::from_bytes(&bytes)
            {
                let _ = stream.update_params(&mut [pod]);
            }
        })
        .process(|stream, state| {
            let Some((name, width, height)) = state.format else {
                return;
            };
            let Some(mut buffer) = stream.dequeue_buffer() else {
                return;
            };
            let Some(data) = buffer.datas_mut().first_mut() else {
                return;
            };
            if let Some(pixels) = read_frame(data, width, height) {
                let _ = state.sender.try_send(Event::Frame(Frame {
                    format: FrameFormat {
                        name,
                        width,
                        height,
                    },
                    data: pixels,
                }));
            }
        })
        .register()
        .context("无法注册窗口视频流回调")?;
    let format_bytes = format_param()?;
    let pod = Pod::from_bytes(&format_bytes).context("窗口视频格式参数无效")?;
    stream
        .connect(
            Direction::Input,
            Some(node_id),
            StreamFlags::AUTOCONNECT,
            &mut [pod],
        )
        .context("无法连接窗口视频流")?;
    while !stop.load(Ordering::Relaxed) {
        mainloop.loop_().iterate(Duration::from_millis(50));
        if let Some(message) = failure.lock().unwrap().take() {
            return Err(anyhow!(message));
        }
    }
    let _ = stream.disconnect();
    Ok(())
}

fn serialize(object: Object) -> Result<Vec<u8>> {
    let (cursor, _) = PodSerializer::serialize(Cursor::new(Vec::new()), &Value::Object(object))
        .map_err(|error| anyhow!("无法序列化 PipeWire 参数: {error:?}"))?;
    Ok(cursor.into_inner())
}

fn id_property(key: FormatProperties, value: u32) -> Property {
    Property::new(key.as_raw(), Value::Id(Id(value)))
}

fn format_param() -> Result<Vec<u8>> {
    let formats = Choice(
        ChoiceFlags::empty(),
        ChoiceEnum::Enum {
            default: Id(VideoFormat::BGRx.as_raw()),
            alternatives: vec![
                Id(VideoFormat::BGRx.as_raw()),
                Id(VideoFormat::BGRA.as_raw()),
            ],
        },
    );
    let modifiers = Choice(
        ChoiceFlags::empty(),
        ChoiceEnum::Enum {
            default: LINEAR_MODIFIER,
            alternatives: vec![LINEAR_MODIFIER],
        },
    );
    let size = Choice(
        ChoiceFlags::empty(),
        ChoiceEnum::Range {
            default: Rectangle {
                width: 1280,
                height: 720,
            },
            min: Rectangle {
                width: 1,
                height: 1,
            },
            max: Rectangle {
                width: 16384,
                height: 16384,
            },
        },
    );
    let framerate = Choice(
        ChoiceFlags::empty(),
        ChoiceEnum::Range {
            default: Fraction { num: 60, denom: 1 },
            min: Fraction { num: 0, denom: 1 },
            max: Fraction {
                num: 1000,
                denom: 1,
            },
        },
    );
    serialize(Object {
        type_: SpaTypes::ObjectParamFormat.as_raw(),
        id: ParamType::EnumFormat.as_raw(),
        properties: vec![
            id_property(FormatProperties::MediaType, MediaType::Video.as_raw()),
            id_property(FormatProperties::MediaSubtype, MediaSubtype::Raw.as_raw()),
            Property::new(
                FormatProperties::VideoFormat.as_raw(),
                Value::Choice(ChoiceValue::Id(formats)),
            ),
            Property {
                key: FormatProperties::VideoModifier.as_raw(),
                flags: PropertyFlags::MANDATORY | PropertyFlags::DONT_FIXATE,
                value: Value::Choice(ChoiceValue::Long(modifiers)),
            },
            Property::new(
                FormatProperties::VideoSize.as_raw(),
                Value::Choice(ChoiceValue::Rectangle(size)),
            ),
            Property::new(
                FormatProperties::VideoFramerate.as_raw(),
                Value::Choice(ChoiceValue::Fraction(framerate)),
            ),
        ],
    })
}

fn buffers_param() -> Result<Vec<u8>> {
    let data_types = (1 << DataType::DmaBuf.as_raw()) | (1 << DataType::MemFd.as_raw());
    serialize(Object {
        type_: SpaTypes::ObjectParamBuffers.as_raw(),
        id: ParamType::Buffers.as_raw(),
        properties: vec![Property::new(
            spa_sys::SPA_PARAM_BUFFERS_dataType,
            Value::Int(data_types),
        )],
    })
}

fn read_frame(data: &mut Data, width: u32, height: u32) -> Option<Vec<u8>> {
    let raw = data.as_raw();
    let kind = data.type_();
    if kind != DataType::DmaBuf && kind != DataType::MemFd {
        return None;
    }
    let (offset, size, stride) = {
        let chunk = data.chunk();
        (
            chunk.offset() as usize,
            chunk.size() as usize,
            chunk.stride() as usize,
        )
    };
    if size == 0 {
        return None;
    }
    let row_bytes = width as usize * 4;
    let stride = if stride == 0 { row_bytes } else { stride };
    let total = offset + stride * (height as usize - 1) + row_bytes;
    let map_len = raw.mapoffset as usize + (raw.maxsize as usize).max(total);
    let fd = data.fd();
    let mapped = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            map_len,
            libc::PROT_READ,
            libc::MAP_SHARED,
            fd.as_raw_fd(),
            0,
        )
    };
    if mapped == libc::MAP_FAILED {
        return None;
    }
    let dma = kind == DataType::DmaBuf;
    sync(fd, DMA_BUF_SYNC_START | DMA_BUF_SYNC_READ, dma);
    let base = unsafe { (mapped as *const u8).add(raw.mapoffset as usize + offset) };
    let mut pixels = vec![0u8; row_bytes * height as usize];
    for row in 0..height as usize {
        let source = unsafe { std::slice::from_raw_parts(base.add(row * stride), row_bytes) };
        pixels[row * row_bytes..(row + 1) * row_bytes].copy_from_slice(source);
    }
    sync(fd, DMA_BUF_SYNC_END | DMA_BUF_SYNC_READ, dma);
    unsafe { libc::munmap(mapped, map_len) };
    Some(pixels)
}

fn sync(fd: i32, flags: u64, enabled: bool) {
    if enabled {
        let request = DmaBufSync { flags };
        unsafe { libc::ioctl(fd, DMA_BUF_IOCTL_SYNC, &request) };
    }
}
