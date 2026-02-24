mod command_utils;
mod output;
mod recording;
mod screenshot;
mod state;
mod windows;

use std::path::PathBuf;
use std::process::Child;

pub use recording::{
    start_recording, start_recording_detached, stop_recording, stop_recording_detached,
    toggle_recording_pause,
};
pub use screenshot::{
    is_window_protocol_unsupported_error, take_screenshot, take_window_screenshot,
    take_window_screenshot_via_niri,
};
pub use windows::{focused_output_name, list_windows};

#[derive(Clone, Copy)]
pub enum CaptureTarget {
    Region,
    Fullscreen,
}

impl CaptureTarget {
    pub(crate) fn slug(self) -> &'static str {
        match self {
            CaptureTarget::Region => "region",
            CaptureTarget::Fullscreen => "fullscreen",
        }
    }
}

#[derive(Clone, Debug)]
pub struct WindowInfo {
    pub id: u64,
    pub title: String,
    pub app_id: String,
    pub workspace_id: u64,
    pub is_focused: bool,
}

pub struct RecordingSession {
    pub(crate) child: Child,
    pub(crate) output_path: PathBuf,
    pub(crate) paused: bool,
}
