use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

const HELPER_SOURCE: &str = include_str!("ocr_helper.py");

#[derive(Debug)]
pub struct OcrOutcome {
    pub lines: Vec<String>,
}

impl OcrOutcome {
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }
}

pub struct PaddleOcrEngine {
    python: PathBuf,
    helper: PathBuf,
}

impl PaddleOcrEngine {
    pub fn resolve() -> Result<Self> {
        let python = std::env::var_os("NCAPTURA_OCR_PYTHON")
            .map(PathBuf::from)
            .or_else(|| {
                crate::config::get()
                    .ocr
                    .python
                    .as_ref()
                    .map(|path| crate::config::expand_tilde(path))
            })
            .or_else(default_venv_python)
            .filter(|path| path.is_file())
            .context(
                "未找到 OCR Python 环境，请先执行 `ncaptura ocr setup`，或设置 NCAPTURA_OCR_PYTHON",
            )?;

        let helper = materialize_helper()?;

        Ok(Self { python, helper })
    }

    pub fn recognize(&self, image: &Path) -> Result<OcrOutcome> {
        let output = Command::new(&self.python)
            .arg(&self.helper)
            .arg(image)
            .env("PADDLE_PDX_CACHE_HOME", model_dir())
            .output()
            .context("无法启动 OCR 识别进程")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let payload = last_non_empty_line(&stdout).context("OCR 进程没有输出识别结果")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let detail = stderr
                .lines()
                .rev()
                .find(|line| !line.trim().is_empty())
                .unwrap_or("未知错误");
            bail!("OCR 识别失败: {detail}");
        }

        parse_helper_output(payload)
    }
}

pub fn setup_ocr_environment() -> Result<()> {
    let (program, version) = find_system_python()?;
    eprintln!("==> 使用系统 Python: {} ({version})", program.display());

    let venv_dir = default_venv_dir().context("无法确定 OCR 环境目录")?;
    let venv_python = crate::platform::python::venv_python(&venv_dir);

    eprintln!("==> 创建 Python 虚拟环境: {}", venv_dir.display());
    run_inherited(
        Command::new(&program)
            .args(["-m", "venv", "--clear"])
            .arg(&venv_dir),
        "创建虚拟环境失败",
    )?;

    eprintln!("==> 安装 paddlepaddle 3.2.2 与 paddleocr");
    run_inherited(
        Command::new(&venv_python).args([
            "-m",
            "pip",
            "install",
            "paddlepaddle==3.2.2",
            "paddleocr>=3,<4",
        ]),
        "安装 PaddleOCR 失败",
    )?;

    let models = model_dir();
    fs::create_dir_all(&models).context("无法创建模型目录")?;
    run_inherited(
        Command::new(&venv_python)
            .args(["-c", "import paddle, paddleocr"])
            .env("PADDLE_PDX_CACHE_HOME", &models),
        "OCR 环境校验失败",
    )?;

    eprintln!("OCR 环境已就绪: {}", venv_dir.display());
    eprintln!("模型将在首次识别时下载到: {}", models.display());
    Ok(())
}

fn find_system_python() -> Result<(PathBuf, String)> {
    for candidate in crate::platform::python::interpreter_candidates() {
        let Ok(output) = Command::new(&candidate)
            .args([
                "-c",
                "import sys; print(f'{sys.version_info.major}.{sys.version_info.minor}')",
            ])
            .output()
        else {
            continue;
        };
        if !output.status.success() {
            continue;
        }

        let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if supported_python_version(&version) {
            return Ok((candidate, version));
        }
    }

    bail!(
        "未找到可用的系统 Python（需要 3.10–3.13）。\
        paddlepaddle 官方暂未发布 Python 3.14 的预编译包，\
        请先安装 python3.10–3.13 之一（如 sudo pacman -S python310）"
    )
}

fn supported_python_version(version: &str) -> bool {
    let Some((major, minor)) = version.split_once('.') else {
        return false;
    };
    major == "3" && matches!(minor.parse::<u32>(), Ok(10..=13))
}

pub fn model_dir() -> PathBuf {
    if let Some(configured) = &crate::config::get().ocr.model_dir {
        return crate::config::expand_tilde(configured);
    }
    dirs::data_dir()
        .map(|dir| dir.join("ncaptura/models"))
        .unwrap_or_else(|| PathBuf::from("~/.local/share/ncaptura/models"))
}

fn run_inherited(command: &mut Command, context_message: &str) -> Result<()> {
    let status = command
        .status()
        .with_context(|| format!("{context_message}: 无法启动命令"))?;
    if !status.success() {
        bail!("{context_message}: 退出码 {}", status);
    }
    Ok(())
}

fn default_venv_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|dir| dir.join("ncaptura/ocr-venv"))
}

fn default_venv_python() -> Option<PathBuf> {
    default_venv_dir().map(|dir| crate::platform::python::venv_python(&dir))
}

fn materialize_helper() -> Result<PathBuf> {
    let cache_dir = dirs::cache_dir()
        .context("无法确定缓存目录")?
        .join("ncaptura");
    fs::create_dir_all(&cache_dir).context("无法创建 OCR 缓存目录")?;

    let helper_path = cache_dir.join("ocr_helper.py");
    let existing = fs::read_to_string(&helper_path).ok();
    if existing.as_deref() != Some(HELPER_SOURCE) {
        fs::write(&helper_path, HELPER_SOURCE).context("无法写入 OCR 辅助脚本")?;
    }

    Ok(helper_path)
}

fn last_non_empty_line(output: &str) -> Option<&str> {
    output.lines().rev().find(|line| !line.trim().is_empty())
}

fn parse_helper_output(payload: &str) -> Result<OcrOutcome> {
    let value: serde_json::Value =
        serde_json::from_str(payload).context("OCR 输出不是有效 JSON")?;

    if let Some(error) = value.get("error").and_then(|item| item.as_str()) {
        bail!("OCR 识别失败: {error}");
    }

    let lines = value
        .get("lines")
        .and_then(|item| item.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    Ok(OcrOutcome { lines })
}

#[cfg(test)]
mod tests {
    use super::{last_non_empty_line, parse_helper_output};

    #[test]
    fn parses_lines_payload() {
        let outcome = parse_helper_output(r#"{"lines": ["第一行", "第二行"]}"#).unwrap();
        assert_eq!(outcome.lines, vec!["第一行", "第二行"]);
        assert_eq!(outcome.text(), "第一行\n第二行");
    }

    #[test]
    fn rejects_error_payload() {
        let err = parse_helper_output(r#"{"error": "模型加载失败"}"#).unwrap_err();
        assert!(err.to_string().contains("模型加载失败"));
    }

    #[test]
    #[ignore = "需要真实 OCR Python 环境与测试图片，通过 NCAPTURA_OCR_TEST_IMAGE 显式运行"]
    fn recognizes_real_image_with_paddle_engine() {
        let image = std::env::var_os("NCAPTURA_OCR_TEST_IMAGE")
            .expect("设置 NCAPTURA_OCR_TEST_IMAGE 指向包含文字的测试图片");
        let engine = super::PaddleOcrEngine::resolve().unwrap();
        let outcome = engine.recognize(std::path::Path::new(&image)).unwrap();
        assert!(!outcome.text().trim().is_empty());
    }

    #[test]
    fn rejects_non_json_payload() {
        assert!(parse_helper_output("not json").is_err());
    }

    #[test]
    fn takes_last_non_empty_line() {
        assert_eq!(
            last_non_empty_line("noise\n{\"lines\": []}\n\n"),
            Some("{\"lines\": []}")
        );
        assert_eq!(last_non_empty_line(" \n\n"), None);
    }
}
