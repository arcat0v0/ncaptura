use std::path::PathBuf;
use std::process::Command;

use anyhow::Result;

use crate::capture::command_utils::{copy_image_to_clipboard, pick_region_geometry, run_command};
use crate::capture::output::build_output_path;
use crate::capture::{CaptureTarget, focused_output_name};

pub fn take_screenshot(target: CaptureTarget) -> Result<PathBuf> {
    take_screenshot_with_clipboard(target, false)
}

pub fn take_screenshot_with_clipboard(
    target: CaptureTarget,
    copy_to_clipboard: bool,
) -> Result<PathBuf> {
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

    if copy_to_clipboard {
        copy_image_to_clipboard(&output_path)?;
    }

    Ok(output_path)
}

pub fn take_window_screenshot(window_id: u64, copy_to_clipboard: bool) -> Result<PathBuf> {
    let output_path = build_output_path(
        "screenshots",
        &format!("screenshot-window-{window_id}"),
        "png",
    )?;

    let mut command = Command::new("grim");
    command.args(["-T", &window_id.to_string()]);
    command.arg(&output_path);
    run_command(command, "截图失败")?;

    if copy_to_clipboard {
        copy_image_to_clipboard(&output_path)?;
    }

    Ok(output_path)
}

pub fn take_window_screenshot_via_niri(window_id: u64) -> Result<()> {
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

pub fn is_window_protocol_unsupported_error(err: &anyhow::Error) -> bool {
    err.to_string()
        .contains("compositor doesn't support the screen capture protocol")
}
