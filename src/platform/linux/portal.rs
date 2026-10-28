use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use ashpd::desktop::screenshot::Screenshot;

const PORTAL_RESPONSE_TIMEOUT: Duration = Duration::from_secs(120);

pub fn capture_screenshot(interactive: bool, output_path: &Path) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("无法初始化截图运行时")?;
    eprintln!("等待 xdg-desktop-portal 截图响应…");
    runtime.block_on(async move {
        let response = tokio::time::timeout(PORTAL_RESPONSE_TIMEOUT, async {
            Screenshot::request()
                .interactive(interactive)
                .send()
                .await
                .context("无法连接 Screenshot portal")?
                .response()
                .context("截图请求被拒绝或失败")
        })
        .await
        .map_err(|_| anyhow!("截图请求超时：portal 未在 120 秒内响应"))??;

        let source = file_uri_to_path(response.uri().as_str())?;
        fs::copy(&source, output_path).with_context(|| {
            format!(
                "portal 截图保存失败: {} -> {}",
                source.display(),
                output_path.display()
            )
        })?;
        let _ = fs::remove_file(&source);
        Ok(())
    })
}

fn file_uri_to_path(uri: &str) -> Result<PathBuf> {
    let Some(rest) = uri.strip_prefix("file://") else {
        bail!("portal 返回的不是文件地址: {uri}");
    };
    let Some(path) = rest.strip_prefix('/') else {
        bail!("portal 截图地址指向远程主机: {uri}");
    };
    percent_decode(&format!("/{path}"))
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("portal 截图地址包含无效的百分号转义: {uri}"))
}

fn percent_decode(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' => {
                let hex = bytes.get(index + 1..index + 3)?;
                let high = (hex[0] as char).to_digit(16)?;
                let low = (hex[1] as char).to_digit(16)?;
                decoded.push((high * 16 + low) as u8);
                index += 3;
            }
            byte => {
                decoded.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(decoded).ok()
}

#[cfg(test)]
mod tests {
    use super::file_uri_to_path;
    use std::path::PathBuf;

    #[test]
    fn parses_plain_file_uri() {
        assert_eq!(
            file_uri_to_path("file:///tmp/shot.png").unwrap(),
            PathBuf::from("/tmp/shot.png")
        );
    }

    #[test]
    fn decodes_percent_escapes() {
        assert_eq!(
            file_uri_to_path("file:///tmp/a%20b%2Fc.png").unwrap(),
            PathBuf::from("/tmp/a b/c.png")
        );
    }

    #[test]
    fn rejects_non_file_and_remote_uris() {
        assert!(file_uri_to_path("https://example.com/a.png").is_err());
        assert!(file_uri_to_path("file://host/tmp/a.png").is_err());
        assert!(file_uri_to_path("file:///tmp/a%.png").is_err());
    }

    #[test]
    #[ignore = "需要真实桌面会话中可用的 Screenshot portal"]
    fn portal_screenshot_writes_file() {
        let dir = std::env::temp_dir().join(format!("ncaptura-portal-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("shot.png");
        super::capture_screenshot(false, &target).unwrap();
        assert!(target.is_file());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
