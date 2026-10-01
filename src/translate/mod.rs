mod mozhi;

use std::sync::Arc;

use crate::text::is_cjk_char;

pub trait Translator: Send + Sync {
    fn translate(&self, text: &str, target: Option<&str>) -> anyhow::Result<String>;
}

pub fn default_translator() -> Arc<dyn Translator> {
    let config = &crate::config::get().translate;

    if config.backend != "mozhi" {
        eprintln!("未知的翻译后端 {}，回退到 mozhi", config.backend);
    }

    let instances = config
        .mozhi_url
        .as_ref()
        .map(|url| vec![url.trim_end_matches('/').to_string()])
        .unwrap_or_else(mozhi::default_instances);

    Arc::new(mozhi::MozhiTranslator::new(
        instances,
        config.engine.clone(),
    ))
}

pub(crate) fn target_language_for(text: &str, explicit: Option<&str>) -> String {
    if let Some(target) = explicit {
        return target.to_string();
    }

    if let Some(target) = &crate::config::get().translate.target {
        return target.clone();
    }

    if text.chars().any(is_cjk_char) {
        "en".into()
    } else {
        "zh-CN".into()
    }
}

#[cfg(test)]
mod tests {
    use super::target_language_for;

    #[test]
    fn explicit_target_wins() {
        assert_eq!(target_language_for("你好", Some("ja")), "ja");
        assert_eq!(target_language_for("hello", Some("ja")), "ja");
    }

    #[test]
    fn cjk_text_targets_english() {
        assert_eq!(target_language_for("你好，世界", None), "en");
    }

    #[test]
    fn latin_text_targets_chinese() {
        assert_eq!(target_language_for("hello world", None), "zh-CN");
    }
}
