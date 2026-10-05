use std::path::Path;
use std::process::{Child, Command};

use anyhow::{Context, Result};

use super::windowing::focused_output_name;
use crate::platform::Region;

pub fn spawn_fullscreen(with_audio: bool, output_path: &Path) -> Result<Child> {
    let mut command = Command::new("wf-recorder");
    if let Ok(output_name) = focused_output_name() {
        command.args(["-o", &output_name]);
    }
    spawn(command, with_audio, output_path)
}

pub fn spawn_region(region: &Region, with_audio: bool, output_path: &Path) -> Result<Child> {
    let mut command = Command::new("wf-recorder");
    command.args(["-g", &region.to_slurp_geometry()]);
    spawn(command, with_audio, output_path)
}

fn spawn(mut command: Command, with_audio: bool, output_path: &Path) -> Result<Child> {
    if with_audio {
        if let Some(audio_device) = default_system_mix_audio_device() {
            command.arg(format!("--audio={audio_device}"));
        } else {
            command.arg("--audio");
        }
    }

    command.arg("-f").arg(output_path);
    command
        .spawn()
        .context("无法启动 wf-recorder，请确认已安装并在 PATH 中")
}

fn default_system_mix_audio_device() -> Option<String> {
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
