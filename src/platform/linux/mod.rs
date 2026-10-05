pub mod clipboard;
pub mod process;
pub mod python;
pub mod recording;
pub mod region;
pub mod screenshot;
pub mod windowing;

use std::process::Command;

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
