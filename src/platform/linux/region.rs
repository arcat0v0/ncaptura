use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};

use crate::platform::Region;

pub fn pick_region() -> Result<Region> {
    let output = Command::new("slurp")
        .output()
        .context("无法启动 slurp，请确认已安装")?;

    if !output.status.success() {
        bail!("区域选择已取消或 slurp 执行失败");
    }

    let geometry = String::from_utf8(output.stdout).context("slurp 输出不是有效文本")?;
    let geometry = geometry.trim();
    if geometry.is_empty() {
        bail!("未获取到区域坐标");
    }

    parse_slurp_geometry(geometry)
}

fn parse_slurp_geometry(input: &str) -> Result<Region> {
    let parse = || -> Option<Region> {
        let (position, size) = input.split_once(' ')?;
        let (x, y) = position.split_once(',')?;
        let (width, height) = size.split_once('x')?;
        Some(Region {
            x: x.parse().ok()?,
            y: y.parse().ok()?,
            width: width.parse().ok()?,
            height: height.parse().ok()?,
        })
    };
    parse().ok_or_else(|| anyhow!("无法解析区域坐标: {input}"))
}

#[cfg(test)]
mod tests {
    use super::parse_slurp_geometry;
    use crate::platform::Region;

    #[test]
    fn parses_positive_coordinates() {
        assert_eq!(
            parse_slurp_geometry("100,200 800x600").unwrap(),
            Region {
                x: 100,
                y: 200,
                width: 800,
                height: 600
            }
        );
    }

    #[test]
    fn parses_negative_coordinates() {
        assert_eq!(
            parse_slurp_geometry("-1920,0 1920x1080").unwrap(),
            Region {
                x: -1920,
                y: 0,
                width: 1920,
                height: 1080
            }
        );
    }

    #[test]
    fn rejects_invalid_geometry() {
        for input in [
            "abc",
            "",
            "100,200",
            "1,2 3x4x5",
            "1,2 -3x4",
            "2147483648,0 1x1",
            "0,0 4294967296x1",
        ] {
            assert_eq!(
                parse_slurp_geometry(input).unwrap_err().to_string(),
                format!("无法解析区域坐标: {input}")
            );
        }
    }

    #[test]
    fn geometry_round_trips() {
        for region in [
            Region {
                x: 100,
                y: 200,
                width: 800,
                height: 600,
            },
            Region {
                x: -1920,
                y: 0,
                width: 1920,
                height: 1080,
            },
            Region {
                x: i32::MIN,
                y: i32::MAX,
                width: u32::MAX,
                height: 0,
            },
        ] {
            assert_eq!(
                parse_slurp_geometry(&region.to_slurp_geometry()).unwrap(),
                region
            );
        }
    }
}
