use std::fs::File;
use std::io;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

pub(crate) fn run_command(mut command: Command, context_message: &str) -> Result<()> {
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

pub(crate) fn pick_region_geometry() -> Result<String> {
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

pub(crate) fn default_system_mix_audio_device() -> Option<String> {
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

pub(crate) fn copy_image_to_clipboard(path: &Path) -> Result<()> {
    let mut child = Command::new("wl-copy")
        .arg("--type")
        .arg("image/png")
        .stdin(Stdio::piped())
        .spawn()
        .context("无法启动 wl-copy，请确认已安装")?;

    let mut child_stdin = child.stdin.take().context("无法写入 wl-copy 输入流")?;
    let mut image_file =
        File::open(path).with_context(|| format!("无法读取截图文件: {}", path.display()))?;

    io::copy(&mut image_file, &mut child_stdin).context("写入剪贴板数据失败")?;
    drop(child_stdin);

    let status = child.wait().context("等待 wl-copy 结束失败")?;
    if !status.success() {
        bail!("截图已保存，但复制到剪贴板失败");
    }

    Ok(())
}
