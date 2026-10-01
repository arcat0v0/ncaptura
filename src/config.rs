use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub output_dir: Option<PathBuf>,
    pub ocr: OcrConfig,
    pub translate: TranslateConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct OcrConfig {
    pub python: Option<PathBuf>,
    pub model_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct TranslateConfig {
    pub backend: String,
    pub mozhi_url: Option<String>,
    pub engine: String,
    pub target: Option<String>,
}

impl Default for TranslateConfig {
    fn default() -> Self {
        Self {
            backend: "mozhi".into(),
            mozhi_url: None,
            engine: "google".into(),
            target: None,
        }
    }
}

impl Config {
    fn load() -> Self {
        let path = config_path();
        let Ok(text) = fs::read_to_string(&path) else {
            return Self::default();
        };

        match parse_config(&text) {
            Ok(config) => config,
            Err(err) => {
                eprintln!("配置文件 {} 解析失败: {err}，使用默认配置", path.display());
                Self::default()
            }
        }
    }
}

fn parse_config(text: &str) -> Result<Config, serde_yml::Error> {
    serde_yml::from_str(text)
}

pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("ncaptura/config.yaml")
}

static CONFIG: LazyLock<Config> = LazyLock::new(Config::load);

pub fn get() -> &'static Config {
    &CONFIG
}

pub(crate) fn expand_tilde(path: &Path) -> PathBuf {
    let Some(home) = dirs::home_dir() else {
        return path.to_path_buf();
    };
    expand_tilde_with(&home, path)
}

fn expand_tilde_with(home: &Path, path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    if text == "~" {
        return home.to_path_buf();
    }
    if let Some(rest) = text.strip_prefix("~/") {
        return home.join(rest);
    }
    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::{expand_tilde_with, parse_config};
    use std::path::{Path, PathBuf};

    #[test]
    fn empty_config_uses_defaults() {
        let config = parse_config("").unwrap();
        assert!(config.output_dir.is_none());
        assert!(config.ocr.python.is_none());
        assert_eq!(config.translate.backend, "mozhi");
        assert_eq!(config.translate.engine, "google");
        assert!(config.translate.mozhi_url.is_none());
        assert!(config.translate.target.is_none());
    }

    #[test]
    fn parses_full_config() {
        let config = parse_config(
            r#"
output_dir: ~/Captures
ocr:
  python: /opt/ocr/bin/python
translate:
  backend: mozhi
  mozhi_url: http://127.0.0.1:3000
  engine: duckduckgo
  target: ja
"#,
        )
        .unwrap();

        assert_eq!(config.output_dir, Some(PathBuf::from("~/Captures")));
        assert_eq!(
            config.ocr.python,
            Some(PathBuf::from("/opt/ocr/bin/python"))
        );
        assert_eq!(
            config.translate.mozhi_url.as_deref(),
            Some("http://127.0.0.1:3000")
        );
        assert_eq!(config.translate.engine, "duckduckgo");
        assert_eq!(config.translate.target.as_deref(), Some("ja"));
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let config = parse_config("future_option: 1\n").unwrap();
        assert!(config.output_dir.is_none());
    }

    #[test]
    fn invalid_yaml_is_an_error() {
        assert!(parse_config("output_dir: [unclosed").is_err());
    }

    #[test]
    fn expands_tilde_prefixes() {
        let home = Path::new("/home/tester");
        assert_eq!(
            expand_tilde_with(home, Path::new("~")),
            PathBuf::from("/home/tester")
        );
        assert_eq!(
            expand_tilde_with(home, Path::new("~/Captures")),
            PathBuf::from("/home/tester/Captures")
        );
        assert_eq!(
            expand_tilde_with(home, Path::new("/abs/path")),
            PathBuf::from("/abs/path")
        );
        assert_eq!(
            expand_tilde_with(home, Path::new("relative/dir")),
            PathBuf::from("relative/dir")
        );
    }
}
