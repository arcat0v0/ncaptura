mod mozhi;

use crate::text::is_cjk_char;

pub trait Translator {
    fn translate(&self, text: &str) -> anyhow::Result<String>;
}

pub fn default_translator() -> Box<dyn Translator> {
    match std::env::var("NCAPTURA_TRANSLATE_BACKEND").as_deref() {
        Ok(backend) if !backend.trim().is_empty() && backend != "mozhi" => {
            eprintln!("未知的翻译后端 {backend}，回退到 mozhi");
            Box::new(mozhi::MozhiTranslator::from_env())
        }
        _ => Box::new(mozhi::MozhiTranslator::from_env()),
    }
}

pub(crate) fn target_language_for(text: &str) -> String {
    if let Ok(target) = std::env::var("NCAPTURA_TRANSLATE_TARGET")
        && !target.trim().is_empty()
    {
        return target;
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
    fn cjk_text_targets_english() {
        assert_eq!(target_language_for("你好，世界"), "en");
    }

    #[test]
    fn latin_text_targets_chinese() {
        assert_eq!(target_language_for("hello world"), "zh-CN");
    }
}
