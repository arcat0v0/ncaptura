mod cli_recording_hud;
mod interactive_dialog;
mod recording_hud;
mod result_window;
mod save_dialog;
mod translate_input;
mod window_picker;

use std::path::{Path, PathBuf};

use adw::prelude::*;
use anyhow::{Result, bail};

pub use cli_recording_hud::run_cli_recording_hud;
pub use interactive_dialog::{CaptureMode, InteractiveDialogResult, build_interactive_dialog};
pub use save_dialog::build_save_dialog;
pub use window_picker::show_window_picker;

use result_window::{LeftPane, ResultWindowConfig, apply_result_window_css, build_result_window};
use translate_input::build_translate_input_window;

use crate::ocr::PaddleOcrEngine;
use crate::translate::default_translator;

pub fn run_ocr_window(image: PathBuf) {
    run_tool_window("ocr", move |app| {
        let config = ResultWindowConfig {
            title: "文字识别",
            left_label: "原始截图",
            right_label: "识别结果",
            loading_label: "正在识别文字…",
            error_title: "识别失败",
            namespace: "ncaptura-ocr",
            left: LeftPane::Screenshot(image.clone()),
        };
        let task_image = image.clone();
        build_result_window(app, config, move || recognize_text(&task_image));
    });
}

pub fn run_translate_selection_window(source: String) {
    run_tool_window("translate", move |app| {
        let config = translate_config(LeftPane::SourceText(source.clone()));
        let task_source = source.clone();
        build_result_window(app, config, move || {
            default_translator().translate(&task_source)
        });
    });
}

pub fn run_translate_region_window(image: PathBuf) {
    run_tool_window("translate", move |app| {
        let config = translate_config(LeftPane::Screenshot(image.clone()));
        let task_image = image.clone();
        build_result_window(app, config, move || {
            let text = recognize_text(&task_image)?;
            default_translator().translate(&text)
        });
    });
}

pub fn run_translate_input_window() {
    run_tool_window("translate-input", move |app| {
        build_translate_input_window(app, move |app, text| {
            let config = translate_config(LeftPane::SourceText(text.clone()));
            build_result_window(app, config, move || default_translator().translate(&text));
        });
    });
}

fn translate_config(left: LeftPane) -> ResultWindowConfig {
    ResultWindowConfig {
        title: "翻译",
        left_label: match &left {
            LeftPane::Screenshot(_) => "原始截图",
            LeftPane::SourceText(_) => "原文",
        },
        right_label: "翻译结果",
        loading_label: "正在翻译…",
        namespace: "ncaptura-translate",
        error_title: "翻译失败",
        left,
    }
}

fn recognize_text(image: &Path) -> Result<String> {
    let engine = PaddleOcrEngine::resolve()?;
    let text = engine.recognize(image)?.text();
    if text.trim().is_empty() {
        bail!("未识别到任何文字");
    }
    Ok(text)
}

fn run_tool_window(kind: &str, build: impl Fn(&adw::Application) + 'static) {
    let app_id = format!("io.ncaptura.{kind}");
    let app = adw::Application::builder()
        .application_id(&app_id)
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();

    app.connect_activate(move |app| {
        apply_result_window_css();
        build(app);
    });
    app.connect_window_removed(|app, _| {
        if app.windows().is_empty() {
            app.quit();
        }
    });

    let _ = app.run_with_args(&["ncaptura"]);
}
