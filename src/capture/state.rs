use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde_json::Value;

const CLI_RECORDING_STATE_FILE: &str = "recording.json";

pub(crate) fn write_cli_recording_state(pid: u32, output_path: &Path) -> Result<()> {
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

pub(crate) fn read_cli_recording_state() -> Result<(u32, PathBuf)> {
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

pub(crate) fn clear_cli_recording_state() {
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
