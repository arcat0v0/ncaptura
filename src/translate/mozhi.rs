use std::time::Duration;

use anyhow::{Context, Result, bail};

use super::{Translator, target_language_for};

pub(crate) fn default_instances() -> Vec<String> {
    [
        "https://mozhi.ducks.party",
        "https://mozhi.pussthecat.org",
        "https://mozhi.canine.tools",
    ]
    .iter()
    .map(|instance| instance.to_string())
    .collect()
}

pub struct MozhiTranslator {
    instances: Vec<String>,
    engine: String,
    agent: ureq::Agent,
}

impl MozhiTranslator {
    pub fn new(instances: Vec<String>, engine: String) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(10)))
            .build()
            .into();

        Self {
            instances,
            engine,
            agent,
        }
    }

    fn translate_with(&self, base: &str, text: &str, target: &str) -> Result<String> {
        let mut response = self
            .agent
            .get(format!("{base}/api/translate"))
            .query("engine", &self.engine)
            .query("from", "auto")
            .query("to", target)
            .query("text", text)
            .call()
            .context("请求翻译实例失败")?;

        let body = response
            .body_mut()
            .read_to_string()
            .context("读取翻译响应失败")?;

        parse_translate_response(&body)
    }
}

impl Translator for MozhiTranslator {
    fn translate(&self, text: &str, target: Option<&str>) -> Result<String> {
        let target = target_language_for(text, target);
        let mut errors = Vec::new();

        for base in &self.instances {
            match self.translate_with(base, text, &target) {
                Ok(translated) => return Ok(translated),
                Err(err) => errors.push(format!("{base}: {err:#}")),
            }
        }

        bail!("翻译实例均不可用（{}）", errors.join("；"))
    }
}

fn parse_translate_response(body: &str) -> Result<String> {
    let value: serde_json::Value = serde_json::from_str(body)
        .with_context(|| format!("翻译响应不是有效 JSON: {}", preview(body)))?;

    if let Some(text) = value.get("translated-text").and_then(|item| item.as_str()) {
        return Ok(text.to_string());
    }

    if let Some(error) = value
        .get("error")
        .or_else(|| value.get("message"))
        .and_then(|item| item.as_str())
    {
        bail!("翻译实例返回错误: {error}");
    }

    bail!("翻译响应缺少 translated-text 字段: {}", preview(body))
}

fn preview(body: &str) -> String {
    body.chars().take(120).collect()
}

#[cfg(test)]
mod tests {
    use super::parse_translate_response;
    use crate::translate::Translator;

    #[test]
    fn parses_translated_text() {
        let body = r#"{"engine":"google","detected":"en","translated-text":"你好世界","target_language":"zh-CN"}"#;
        assert_eq!(parse_translate_response(body).unwrap(), "你好世界");
    }

    #[test]
    fn rejects_error_payload() {
        let body = r#"{"error":"engine unavailable"}"#;
        let err = parse_translate_response(body).unwrap_err();
        assert!(err.to_string().contains("engine unavailable"));
    }

    #[test]
    fn rejects_rate_limited_plain_text() {
        let err = parse_translate_response("instance has been rate limited").unwrap_err();
        assert!(err.to_string().contains("rate limited"));
    }

    #[test]
    #[ignore = "需要访问公共 mozhi 实例，网络依赖较强"]
    fn translates_via_public_instance() {
        let translator = super::MozhiTranslator::new(super::default_instances(), "google".into());
        let translated = translator.translate("Hello world", None).unwrap();
        assert!(translated.contains("你好"));
    }
}
