use std::process::Command;

use anyhow::{Context, Result, bail};
use serde_json::Value;

use crate::capture::WindowInfo;

pub fn list_windows() -> Result<Vec<WindowInfo>> {
    let output = Command::new("niri")
        .args(["msg", "--json", "windows"])
        .output()
        .context("无法调用 niri msg windows，请确认正在 niri 会话中")?;

    if !output.status.success() {
        bail!("niri msg windows 执行失败");
    }

    let stdout = String::from_utf8(output.stdout).context("niri windows JSON 输出不是 UTF-8")?;
    let values: Vec<Value> =
        serde_json::from_str(stdout.trim()).context("niri windows JSON 解析失败")?;

    let mut windows = Vec::new();
    for item in values {
        let Some(id) = item.get("id").and_then(Value::as_u64) else {
            continue;
        };

        let title = item
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("(untitled)")
            .to_string();
        let app_id = item
            .get("app_id")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        let workspace_id = item
            .get("workspace_id")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        let is_focused = item
            .get("is_focused")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        windows.push(WindowInfo {
            id,
            title,
            app_id,
            workspace_id,
            is_focused,
        });
    }

    windows.sort_by_key(|w| (!w.is_focused, w.workspace_id, w.title.clone()));
    Ok(windows)
}

pub fn focused_output_name() -> Result<String> {
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
