use std::path::PathBuf;

use anyhow::{Context, Result, bail};

use crate::capture::output::build_output_path;
use crate::capture::state::{
    clear_cli_recording_state, read_cli_recording_state, write_cli_recording_state,
};
use crate::capture::{CaptureTarget, CliRecordingState, RecordingSession};
use crate::platform::{process, recording, region};

pub fn start_recording(target: CaptureTarget, with_audio: bool) -> Result<RecordingSession> {
    let output_path =
        build_output_path("recordings", &format!("recording-{}", target.slug()), "mkv")?;

    let child = match target {
        CaptureTarget::Region => {
            let region = region::pick_region()?;
            recording::spawn_region(&region, with_audio, &output_path)?
        }
        CaptureTarget::Fullscreen => recording::spawn_fullscreen(with_audio, &output_path)?,
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
    if read_cli_recording_state().is_ok() {
        bail!("已有通过 CLI 启动的录屏在进行中，请先停止");
    }

    let output_path =
        build_output_path("recordings", &format!("recording-{}", target.slug()), "mkv")?;
    let child = match target {
        CaptureTarget::Region => {
            let region = region::pick_region()?;
            recording::spawn_region(&region, with_audio, &output_path)?
        }
        CaptureTarget::Fullscreen => recording::spawn_fullscreen(with_audio, &output_path)?,
    };

    let pid = child.id();
    write_cli_recording_state(pid, &output_path)?;
    Ok(CliRecordingState { pid, output_path })
}

pub fn stop_recording_detached() -> Result<PathBuf> {
    let (pid, output_path) = read_cli_recording_state()?;
    process::resume(pid)?;
    process::interrupt(pid)?;

    clear_cli_recording_state();
    Ok(output_path)
}

pub fn current_cli_recording_state() -> Result<CliRecordingState> {
    let (pid, output_path) = read_cli_recording_state()?;
    Ok(CliRecordingState { pid, output_path })
}
