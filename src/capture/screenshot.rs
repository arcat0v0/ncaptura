use std::path::PathBuf;

use anyhow::Result;

use crate::capture::CaptureTarget;
use crate::capture::output::build_output_path;
use crate::platform;

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

    match target {
        CaptureTarget::Region => {
            let region = platform::region::pick_region()?;
            platform::screenshot::capture_region(&region, &output_path)?;
        }
        CaptureTarget::Fullscreen => platform::screenshot::capture_fullscreen(&output_path)?,
    }

    if copy_to_clipboard {
        platform::clipboard::copy_image(&output_path)?;
    }

    Ok(output_path)
}

pub fn take_window_screenshot(window_id: u64, copy_to_clipboard: bool) -> Result<Option<PathBuf>> {
    let output_path = build_output_path(
        "screenshots",
        &format!("screenshot-window-{window_id}"),
        "png",
    )?;

    if !platform::screenshot::capture_window(window_id, &output_path)? {
        return Ok(None);
    }

    if copy_to_clipboard {
        platform::clipboard::copy_image(&output_path)?;
    }

    Ok(Some(output_path))
}
