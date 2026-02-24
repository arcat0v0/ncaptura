use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use chrono::Local;

pub(crate) fn build_output_path(kind_dir: &str, prefix: &str, extension: &str) -> Result<PathBuf> {
    let base_dir = base_output_dir()?;
    let output_dir = base_dir.join(kind_dir);
    fs::create_dir_all(&output_dir)
        .with_context(|| format!("无法创建输出目录: {}", output_dir.display()))?;

    let timestamp = Local::now().format("%Y%m%d-%H%M%S");
    Ok(output_dir.join(format!("{prefix}-{timestamp}.{extension}")))
}

fn base_output_dir() -> Result<PathBuf> {
    if let Some(pictures_dir) = dirs::picture_dir() {
        return Ok(pictures_dir.join("NCaptura"));
    }

    if let Some(home_dir) = dirs::home_dir() {
        return Ok(home_dir.join("Pictures").join("NCaptura"));
    }

    bail!("无法定位用户目录")
}
