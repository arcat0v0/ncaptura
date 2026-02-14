use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command};

use anyhow::{Context, Result, bail};
use chrono::Local;
use nix::errno::Errno;
use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;
use serde_json::Value;

#[derive(Clone, Copy)]
pub enum CaptureTarget {
    Region,
    Fullscreen,
}

pub struct RecordingSession {
    child: Child,
    output_path: PathBuf,
}

const CLI_RECORDING_STATE_FILE: &str = "recording.json";

pub fn take_screenshot(target: CaptureTarget) -> Result<PathBuf> {
    let output_path = build_output_path(
        "screenshots",
        &format!("screenshot-{}", target.slug()),
        "png",
    )?;

    let mut command = Command::new("grim");
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

    command.arg(&output_path);
    run_command(command, "截图失败")?;
    Ok(output_path)
}

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

    Ok(RecordingSession { child, output_path })
}

pub fn stop_recording(mut session: RecordingSession) -> Result<PathBuf> {
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

pub fn start_recording_detached(target: CaptureTarget, with_audio: bool) -> Result<PathBuf> {
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

    write_cli_recording_state(child.id(), &output_path)?;
    Ok(output_path)
}

pub fn stop_recording_detached() -> Result<PathBuf> {
    let (pid, output_path) = read_cli_recording_state()?;
    let process_id = Pid::from_raw(pid as i32);

    if let Err(err) = kill(process_id, Signal::SIGINT)
        && err != Errno::ESRCH
    {
        bail!("发送停止信号失败: {err}");
    }

    clear_cli_recording_state();
    Ok(output_path)
}

impl CaptureTarget {
    fn slug(self) -> &'static str {
        match self {
            CaptureTarget::Region => "region",
            CaptureTarget::Fullscreen => "fullscreen",
        }
    }
}

fn run_command(mut command: Command, context_message: &str) -> Result<()> {
    let output = command
        .output()
        .with_context(|| format!("{context_message}: 无法启动命令"))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr = stderr.trim();
    if stderr.is_empty() {
        bail!("{context_message}: 退出码 {}", output.status);
    }

    bail!("{context_message}: {stderr}");
}

fn pick_region_geometry() -> Result<String> {
    let output = Command::new("slurp")
        .output()
        .context("无法启动 slurp，请确认已安装")?;

    if !output.status.success() {
        bail!("区域选择已取消或 slurp 执行失败");
    }

    let geometry = String::from_utf8(output.stdout).context("slurp 输出不是有效文本")?;
    let geometry = geometry.trim().to_string();

    if geometry.is_empty() {
        bail!("未获取到区域坐标");
    }

    Ok(geometry)
}

fn focused_output_name() -> Result<String> {
    let output = Command::new("niri")
        .args(["msg", "--json", "focused-output"])
        .output()
        .context("无法调用 niri msg，请确认正在 niri 会话中")?;

    if !output.status.success() {
        bail!("niri msg focused-output 执行失败");
    }

    let stdout = String::from_utf8(output.stdout).context("niri JSON 输出不是 UTF-8")?;
    let data: Value = serde_json::from_str(stdout.trim()).context("niri JSON 解析失败")?;

    if let Some(name) = data.get("name").and_then(Value::as_str) {
        return Ok(name.to_string());
    }

    if let Some(name) = data
        .pointer("/Ok/FocusedOutput/name")
        .and_then(Value::as_str)
    {
        return Ok(name.to_string());
    }

    bail!("未从 niri focused-output 返回中找到输出名称")
}

fn build_output_path(kind_dir: &str, prefix: &str, extension: &str) -> Result<PathBuf> {
    let base_dir = base_output_dir()?;
    let output_dir = base_dir.join(kind_dir);
    fs::create_dir_all(&output_dir)
        .with_context(|| format!("无法创建输出目录: {}", output_dir.display()))?;

    let timestamp = Local::now().format("%Y%m%d-%H%M%S");
    Ok(output_dir.join(format!("{prefix}-{timestamp}.{extension}")))
}

fn base_output_dir() -> Result<PathBuf> {
    if let Some(pictures_dir) = dirs::picture_dir() {
        return Ok(pictures_dir.join("NCaptura"));
    }

    if let Some(home_dir) = dirs::home_dir() {
        return Ok(home_dir.join("Pictures").join("NCaptura"));
    }

    bail!("无法定位用户目录")
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

fn write_cli_recording_state(pid: u32, output_path: &PathBuf) -> Result<()> {
    let state_dir = cli_state_dir()?;
    fs::create_dir_all(&state_dir)
        .with_context(|| format!("无法创建状态目录: {}", state_dir.display()))?;

    let file_path = state_dir.join(CLI_RECORDING_STATE_FILE);
    let data = serde_json::json!({
        "pid": pid,
        "output_path": output_path,
    });

    fs::write(&file_path, data.to_string())
        .with_context(|| format!("无法写入状态文件: {}", file_path.display()))?;

    Ok(())
}

fn read_cli_recording_state() -> Result<(u32, PathBuf)> {
    let file_path = cli_state_dir()?.join(CLI_RECORDING_STATE_FILE);
    let data = fs::read_to_string(&file_path)
        .with_context(|| format!("无法读取录屏状态文件: {}", file_path.display()))?;

    let value: Value = serde_json::from_str(&data).context("录屏状态文件解析失败")?;
    let pid = value
        .get("pid")
        .and_then(Value::as_u64)
        .context("录屏状态缺少 pid")? as u32;

    let output_path = value
        .get("output_path")
        .and_then(Value::as_str)
        .context("录屏状态缺少 output_path")?;

    Ok((pid, PathBuf::from(output_path)))
}

fn clear_cli_recording_state() {
    if let Ok(file_path) = cli_state_dir().map(|dir| dir.join(CLI_RECORDING_STATE_FILE)) {
        let _ = fs::remove_file(file_path);
    }
}

fn cli_state_dir() -> Result<PathBuf> {
    if let Some(state_dir) = dirs::state_dir() {
        return Ok(state_dir.join("ncaptura"));
    }

    if let Some(home_dir) = dirs::home_dir() {
        return Ok(home_dir.join(".local").join("state").join("ncaptura"));
    }

    bail!("无法定位状态目录")
}
