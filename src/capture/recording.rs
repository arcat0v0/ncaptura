use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result, bail};
use nix::errno::Errno;
use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;

use crate::capture::command_utils::{default_system_mix_audio_device, pick_region_geometry};
use crate::capture::output::build_output_path;
use crate::capture::state::{
    clear_cli_recording_state, read_cli_recording_state, write_cli_recording_state,
};
use crate::capture::{CaptureTarget, CliRecordingState, RecordingSession, focused_output_name};

pub fn start_recording(target: CaptureTarget, with_audio: bool) -> Result<RecordingSession> {
    let output_path =
        build_output_path("recordings", &format!("recording-{}", target.slug()), "mkv")?;

    let mut command = Command::new("wf-recorder");

    match target {
        CaptureTarget::Region => {
            let geometry = pick_region_geometry()?;
            command.args(["-g", &geometry]);
        }
        CaptureTarget::Fullscreen => {
            if let Ok(output_name) = focused_output_name() {
                command.args(["-o", &output_name]);
            }
        }
    }

    if with_audio {
        if let Some(audio_device) = default_system_mix_audio_device() {
            command.arg(format!("--audio={audio_device}"));
        } else {
            command.arg("--audio");
        }
    }

    command.arg("-f").arg(&output_path);

    let child = command
        .spawn()
        .context("无法启动 wf-recorder，请确认已安装并在 PATH 中")?;

    Ok(RecordingSession {
        child,
        output_path,
        paused: false,
    })
}

pub fn toggle_recording_pause(session: &mut RecordingSession) -> Result<bool> {
    let pid = Pid::from_raw(session.child.id() as i32);

    if session.paused {
        if let Err(err) = kill(pid, Signal::SIGCONT)
            && err != Errno::ESRCH
        {
            bail!("恢复录屏失败: {err}");
        }
        session.paused = false;
        return Ok(false);
    }

    if let Err(err) = kill(pid, Signal::SIGSTOP)
        && err != Errno::ESRCH
    {
        bail!("暂停录屏失败: {err}");
    }

    session.paused = true;
    Ok(true)
}

pub fn stop_recording(mut session: RecordingSession) -> Result<PathBuf> {
    if session.paused {
        let pid = Pid::from_raw(session.child.id() as i32);
        if let Err(err) = kill(pid, Signal::SIGCONT)
            && err != Errno::ESRCH
        {
            bail!("恢复录屏失败: {err}");
        }
        session.paused = false;
    }

    if session
        .child
        .try_wait()
        .context("读取录屏进程状态失败")?
        .is_none()
    {
        let pid = Pid::from_raw(session.child.id() as i32);
        if let Err(err) = kill(pid, Signal::SIGINT)
            && err != Errno::ESRCH
        {
            bail!("发送停止信号失败: {err}");
        }
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
    let mut command = Command::new("wf-recorder");

    match target {
        CaptureTarget::Region => {
            let geometry = pick_region_geometry()?;
            command.args(["-g", &geometry]);
        }
        CaptureTarget::Fullscreen => {
            if let Ok(output_name) = focused_output_name() {
                command.args(["-o", &output_name]);
            }
        }
    }

    if with_audio {
        if let Some(audio_device) = default_system_mix_audio_device() {
            command.arg(format!("--audio={audio_device}"));
        } else {
            command.arg("--audio");
        }
    }

    command.arg("-f").arg(&output_path);

    let child = command
        .spawn()
        .context("无法启动 wf-recorder，请确认已安装并在 PATH 中")?;

    let pid = child.id();
    write_cli_recording_state(pid, &output_path)?;
    Ok(CliRecordingState { pid, output_path })
}

pub fn stop_recording_detached() -> Result<PathBuf> {
    let (pid, output_path) = read_cli_recording_state()?;
    let process_id = Pid::from_raw(pid as i32);

    if let Err(err) = kill(process_id, Signal::SIGCONT)
        && err != Errno::ESRCH
    {
        bail!("发送恢复信号失败: {err}");
    }

    if let Err(err) = kill(process_id, Signal::SIGINT)
        && err != Errno::ESRCH
    {
        bail!("发送停止信号失败: {err}");
    }

    clear_cli_recording_state();
    Ok(output_path)
}

pub fn current_cli_recording_state() -> Result<CliRecordingState> {
    let (pid, output_path) = read_cli_recording_state()?;
    Ok(CliRecordingState { pid, output_path })
}
