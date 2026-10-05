#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "linux")]
pub use linux::{clipboard, process, python, recording, region, screenshot, windowing};

#[derive(Clone, Debug)]
pub struct WindowInfo {
    pub id: u64,
    pub title: String,
    pub app_id: String,
    pub workspace_id: u64,
    pub is_focused: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Region {
    pub fn to_slurp_geometry(self) -> String {
        format!("{},{} {}x{}", self.x, self.y, self.width, self.height)
    }
}
