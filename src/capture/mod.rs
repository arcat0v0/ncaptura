mod output;
mod recording;
mod screenshot;
mod state;

use std::path::PathBuf;
use std::process::Child;

pub use recording::{
    current_cli_recording_state, start_recording, start_recording_detached, stop_recording,
    stop_recording_detached, toggle_recording_pause,
};
pub use screenshot::{take_screenshot, take_window_screenshot};

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

pub struct RecordingSession {
    pub(crate) child: Child,
    pub(crate) output_path: PathBuf,
    pub(crate) paused: bool,
}

#[derive(Clone, Debug)]
pub struct CliRecordingState {
    pub pid: u32,
    pub output_path: PathBuf,
}
