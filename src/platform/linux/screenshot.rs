use std::path::Path;
use std::process::Command;

use anyhow::Result;

use super::{run_command, windowing::focused_output_name};
use crate::platform::Region;

pub fn capture_fullscreen(output_path: &Path) -> Result<()> {
    let mut command = Command::new("grim");
    if let Ok(output_name) = focused_output_name() {
        command.args(["-o", &output_name]);
    }
    command.arg(output_path);
    run_command(command, "截图失败")
}

pub fn capture_region(region: &Region, output_path: &Path) -> Result<()> {
    let mut command = Command::new("grim");
    command.args(["-g", &region.to_slurp_geometry()]);
    command.arg(output_path);
    run_command(command, "截图失败")
}

pub fn capture_window(window_id: u64, output_path: &Path) -> Result<bool> {
    let mut command = Command::new("grim");
    command.args(["-T", &window_id.to_string()]);
    command.arg(output_path);
    match run_command(command, "截图失败") {
        Ok(()) => Ok(true),
        Err(err) if is_window_protocol_unsupported_error(&err) => {
            take_window_screenshot_via_niri(window_id)?;
            Ok(false)
        }
        Err(err) => Err(err),
    }
}

fn take_window_screenshot_via_niri(window_id: u64) -> Result<()> {
    let mut focus = Command::new("niri");
    focus.args([
        "msg",
        "action",
        "focus-window",
        "--id",
        &window_id.to_string(),
    ]);
    run_command(focus, "聚焦目标窗口失败")?;

    let mut screenshot = Command::new("niri");
    screenshot.args(["msg", "action", "screenshot-window"]);
    run_command(screenshot, "niri 窗口截图失败")?;

    Ok(())
}

fn is_window_protocol_unsupported_error(err: &anyhow::Error) -> bool {
    err.to_string()
        .contains("compositor doesn't support the screen capture protocol")
}
