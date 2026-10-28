use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::Child;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

use crate::capture::output::build_output_path;
use crate::capture::state::{
    clear_cli_recording_state, read_cli_recording_state, write_cli_recording_state,
};
use crate::capture::{CaptureTarget, CliRecordingState, RecordingSession};
use crate::platform::capabilities::Capability;
use crate::platform::{capabilities, compositor, process, recording, region};

pub fn start_recording(target: CaptureTarget, with_audio: bool) -> Result<RecordingSession> {
    ensure_recording_supported(target)?;
    let output_path =
        build_output_path("recordings", &format!("recording-{}", target.slug()), "mkv")?;

    let child = match target {
        CaptureTarget::Region => {
            let region = region::pick_region()?;
            recording::spawn_region(&region, with_audio, &output_path)?
        }
        CaptureTarget::Fullscreen => recording::spawn_fullscreen(with_audio, &output_path)?,
        CaptureTarget::Window => recording::spawn_window(with_audio, &output_path)?,
    };

    Ok(RecordingSession {
        child,
        output_path,
        paused: false,
    })
}

pub fn toggle_recording_pause(session: &mut RecordingSession) -> Result<bool> {
    let pid = session.child.id();

    if session.paused {
        process::resume(pid)?;
        session.paused = false;
        return Ok(false);
    }

    process::suspend(pid)?;

    session.paused = true;
    Ok(true)
}

pub fn stop_recording(mut session: RecordingSession) -> Result<PathBuf> {
    if session.paused {
        process::resume(session.child.id())?;
        session.paused = false;
    }

    if session
        .child
        .try_wait()
        .context("读取录屏进程状态失败")?
        .is_none()
    {
        process::interrupt(session.child.id())?;
    }

    let status = session.child.wait().context("等待录屏进程结束失败")?;
    if !status.success() {
        bail!("录屏进程异常退出: {status}");
    }

    Ok(session.output_path)
}

pub fn start_recording_detached(
    target: CaptureTarget,
    with_audio: bool,
) -> Result<CliRecordingState> {
    ensure_recording_supported(target)?;
    if let Ok((pid, _)) = read_cli_recording_state() {
        if process::is_running(pid) {
            bail!("已有通过 CLI 启动的录屏在进行中，请先停止");
        }
        clear_cli_recording_state();
    }

    let output_path =
        build_output_path("recordings", &format!("recording-{}", target.slug()), "mkv")?;
    let mut child = match target {
        CaptureTarget::Region => {
            let region = region::pick_region()?;
            recording::spawn_region(&region, with_audio, &output_path)?
        }
        CaptureTarget::Fullscreen => recording::spawn_fullscreen(with_audio, &output_path)?,
        CaptureTarget::Window => recording::spawn_window(with_audio, &output_path)?,
    };

    let pid = child.id();
    if is_portal_recording(target)
        && let Err(error) = wait_until_portal_capture_ready(&mut child, target)
    {
        let _ = process::interrupt(pid);
        let _ = child.wait();
        return Err(error);
    }
    write_cli_recording_state(pid, &output_path)?;
    Ok(CliRecordingState { pid, output_path })
}

fn is_portal_recording(target: CaptureTarget) -> bool {
    match target {
        CaptureTarget::Window => true,
        CaptureTarget::Fullscreen => {
            matches!(
                compositor::detect(),
                compositor::Compositor::Kde | compositor::Compositor::Gnome
            )
        }
        CaptureTarget::Region => false,
    }
}

fn wait_until_portal_capture_ready(child: &mut Child, target: CaptureTarget) -> Result<()> {
    let target_name = match target {
        CaptureTarget::Window => "窗口",
        CaptureTarget::Fullscreen => "全屏",
        CaptureTarget::Region => "区域",
    };
    let mut reader = BufReader::new(
        child
            .stdout
            .take()
            .with_context(|| format!("{target_name}录屏进程没有状态输出"))?,
    );
    let mut stderr_reader = BufReader::new(
        child
            .stderr
            .take()
            .with_context(|| format!("{target_name}录屏进程没有错误输出"))?,
    );
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        if Instant::now() >= deadline {
            bail!("等待{target_name}选择超时");
        }
        let mut line = String::new();
        let bytes = reader
            .read_line(&mut line)
            .with_context(|| format!("读取{target_name}录屏状态失败"))?;
        if bytes == 0 {
            let status = child
                .wait()
                .with_context(|| format!("等待{target_name}录屏进程失败"))?;
            let mut stderr = String::new();
            let _ = std::io::BufRead::read_line(&mut stderr_reader, &mut stderr);
            let stderr = stderr.trim();
            if stderr.is_empty() {
                bail!("{target_name}录屏在开始前退出: {status}");
            }
            bail!("{target_name}录屏在开始前退出: {status}: {stderr}");
        }
        match line.trim() {
            "" => bail!("{target_name}录屏在开始前退出"),
            "READY" => return Ok(()),
            _ => continue,
        }
    }
}

pub fn stop_recording_detached() -> Result<PathBuf> {
    let (pid, output_path) = read_cli_recording_state()?;
    process::resume(pid)?;
    process::interrupt(pid)?;
    if !process::wait_for_exit(pid, Duration::from_secs(5)) {
        bail!("录屏进程未在超时时间内退出");
    }

    let metadata = std::fs::metadata(&output_path).context("读取录屏文件失败")?;
    if metadata.len() == 0 {
        bail!("录屏文件为空，内容可能未成功捕获");
    }
    clear_cli_recording_state();
    Ok(output_path)
}

pub fn current_cli_recording_state() -> Result<CliRecordingState> {
    let (pid, output_path) = read_cli_recording_state()?;
    if !process::is_running(pid) {
        clear_cli_recording_state();
        bail!("录屏进程已结束");
    }
    Ok(CliRecordingState { pid, output_path })
}

fn ensure_recording_supported(target: CaptureTarget) -> Result<()> {
    let kind = compositor::detect();
    let capability = match target {
        CaptureTarget::Region => Capability::RecordRegion,
        CaptureTarget::Fullscreen => Capability::RecordFullscreen,
        CaptureTarget::Window => Capability::RecordWindow,
    };
    if capabilities::supported(kind, capability) {
        return Ok(());
    }

    bail!(
        "{}暂不支持{}会话，目前仅 niri 等 wlr 协议合成器可用（依赖 wf-recorder）",
        capability.label(),
        kind.label()
    );
}
