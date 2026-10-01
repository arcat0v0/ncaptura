use anyhow::{Result, bail};

pub trait Translator {
    fn translate(&self, text: &str) -> Result<String>;
}

pub struct PendingTranslator;

impl Translator for PendingTranslator {
    fn translate(&self, _text: &str) -> Result<String> {
        bail!("翻译 API 尚未接入，敬请期待")
    }
}

pub fn default_translator() -> impl Translator {
    PendingTranslator
}
