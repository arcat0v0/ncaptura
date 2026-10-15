use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

use super::overlay::{self, OverlayKind};
use adw::prelude::*;
use gtk::{Align, Orientation};

use crate::platform::clipboard::copy_text;
use crate::text::remove_line_breaks;

pub enum LeftPane {
    Screenshot(PathBuf),
    SourceText(String),
}

pub struct ResultWindowConfig {
    pub title: &'static str,
    pub left_label: &'static str,
    pub right_label: &'static str,
    pub loading_label: &'static str,
    pub error_title: &'static str,
    pub namespace: &'static str,
    pub left: LeftPane,
    pub language_selector: bool,
    pub translate_button: bool,
    /// 在页脚显示“重新翻译”按钮，允许用户主动重新发起请求。
    pub retry_button: bool,
}

const TARGET_LANGUAGES: &[(&str, &str)] = &[
    ("", "自动"),
    ("zh-CN", "中文"),
    ("en", "English"),
    ("ja", "日本語"),
    ("ko", "한국어"),
    ("fr", "Français"),
    ("de", "Deutsch"),
    ("ru", "Русский"),
    ("es", "Español"),
];

/// 任务入参：(当前左侧原文，目标语言)。截图类窗口没有原文，传入空字符串。
type SharedTask =
    std::sync::Arc<dyn Fn(String, Option<String>) -> anyhow::Result<String> + Send + Sync>;

const FADE_DURATION_MS: u32 = 160;

/// 结果窗口的共享状态，供语言切换、重试、互换等回调使用。
struct ResultUi {
    stack: gtk::Stack,
    result_view: gtk::TextView,
    /// 仅文本原文的窗口才有可编辑的左侧文本框。
    source_view: Option<gtk::TextView>,
    dropdown: Option<gtk::DropDown>,
    toast_overlay: adw::ToastOverlay,
    task: SharedTask,
    error_title: &'static str,
    /// 每次发起请求自增，用于丢弃过期的响应。
    generation: Cell<u64>,
    /// 互换动画进行中，期间忽略新的翻译请求。
    swapping: Cell<bool>,
    /// 程序化修改语言下拉框时，避免触发翻译。
    suppress_lang_notify: Cell<bool>,
}

fn buffer_text(view: &gtk::TextView) -> String {
    let buffer = view.buffer();
    buffer
        .text(&buffer.start_iter(), &buffer.end_iter(), false)
        .to_string()
}

fn target_for_index(index: usize) -> Option<String> {
    TARGET_LANGUAGES
        .get(index)
        .filter(|(code, _)| !code.is_empty())
        .map(|(code, _)| code.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WindowLayout {
    width: i32,
    height: i32,
    vertical: bool,
    pane_position: i32,
}

impl WindowLayout {
    fn for_monitor(monitor: Option<(i32, i32)>) -> Self {
        let (mon_w, mon_h) = monitor.unwrap_or((1920, 1080));
        let usable_w = (mon_w * 92 / 100).max(360);
        let usable_h = (mon_h * 85 / 100).max(320);

        if mon_h > mon_w {
            let width = usable_w.min(860);
            let height = usable_h.min(1100);
            Self {
                width,
                height,
                vertical: true,
                pane_position: height * 42 / 100,
            }
        } else {
            let width = usable_w.min(1060);
            let height = usable_h.min(620);
            Self {
                width,
                height,
                vertical: false,
                pane_position: width * 44 / 100,
            }
        }
    }
}

pub(crate) fn target_monitor_size() -> Option<(i32, i32)> {
    let display = gtk::gdk::Display::default()?;
    let monitors = display.monitors();
    let focused = crate::platform::windowing::focused_output_name().ok();

    let mut fallback = None;
    for index in 0..monitors.n_items() {
        let Some(monitor) = monitors
            .item(index)
            .and_then(|item| item.downcast::<gtk::gdk::Monitor>().ok())
        else {
            continue;
        };
        if fallback.is_none() {
            fallback = Some(monitor.clone());
        }
        if focused
            .as_deref()
            .is_some_and(|name| monitor.connector().as_deref() == Some(name))
        {
            let geometry = monitor.geometry();
            return Some((geometry.width(), geometry.height()));
        }
    }

    let geometry = fallback?.geometry();
    Some((geometry.width(), geometry.height()))
}

pub fn build_result_window<F, G>(
    app: &adw::Application,
    config: ResultWindowConfig,
    task: F,
    on_translate: Option<G>,
) -> adw::ApplicationWindow
where
    F: Fn(String, Option<String>) -> anyhow::Result<String> + Send + Sync + 'static,
    G: Fn(&adw::Application, String) + 'static,
{
    let layout = WindowLayout::for_monitor(target_monitor_size());
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title(config.title)
        .default_width(layout.width)
        .default_height(layout.height)
        .decorated(false)
        .build();
    window.add_css_class("ncaptura-tool-window");

    overlay::configure(&window, OverlayKind::Tool, config.namespace);
    let toast_overlay = adw::ToastOverlay::new();
    let toolbar_view = adw::ToolbarView::new();

    let header = adw::HeaderBar::builder()
        .title_widget(&adw::WindowTitle::new(config.title, ""))
        .show_start_title_buttons(false)
        .show_end_title_buttons(false)
        .build();
    let close_button = gtk::Button::builder()
        .icon_name("window-close-symbolic")
        .tooltip_text("关闭")
        .build();
    close_button.add_css_class("circular");
    let window_for_close = window.clone();
    close_button.connect_clicked(move |_| window_for_close.close());
    header.pack_end(&close_button);
    toolbar_view.add_top_bar(&header);

    let paned = gtk::Paned::builder()
        .orientation(if layout.vertical {
            Orientation::Vertical
        } else {
            Orientation::Horizontal
        })
        .resize_start_child(false)
        .shrink_start_child(false)
        .resize_end_child(true)
        .shrink_end_child(false)
        .position(layout.pane_position)
        .build();

    let (left_pane, source_view) = build_left_pane(&config);
    paned.set_start_child(Some(&left_pane));
    let (right_pane, stack, text_view, footer) =
        build_right_pane(&config, &toast_overlay, app, on_translate);
    paned.set_end_child(Some(&right_pane));

    toolbar_view.set_content(Some(&paned));
    toast_overlay.set_child(Some(&toolbar_view));
    window.set_content(Some(&toast_overlay));

    let dropdown = config.language_selector.then(|| {
        let labels: Vec<&str> = TARGET_LANGUAGES.iter().map(|(_, label)| *label).collect();
        let dropdown = gtk::DropDown::from_strings(&labels);
        dropdown.set_tooltip_text(Some("目标语言"));
        dropdown
    });

    // 只有可编辑的原文才能与译文互换。
    let swap_button = source_view.as_ref().map(|_| {
        let button = gtk::Button::builder()
            .label("⇄")
            .tooltip_text("互换原文与译文")
            .build();
        button.add_css_class("ncaptura-swap-button");
        button
    });

    let ui = Rc::new(ResultUi {
        stack,
        result_view: text_view,
        source_view,
        dropdown: dropdown.clone(),
        toast_overlay,
        task: std::sync::Arc::new(task),
        error_title: config.error_title,
        generation: Cell::new(0),
        swapping: Cell::new(false),
        suppress_lang_notify: Cell::new(false),
    });

    if let Some(dropdown) = &dropdown {
        let ui_weak = Rc::downgrade(&ui);
        dropdown.connect_selected_notify(move |_| {
            let Some(ui) = ui_weak.upgrade() else { return };
            if ui.suppress_lang_notify.get() {
                return;
            }
            start_task(&ui);
        });
        header.pack_end(dropdown);
    }

    if let Some(button) = &swap_button {
        let ui_weak = Rc::downgrade(&ui);
        button.connect_clicked(move |_| {
            if let Some(ui) = ui_weak.upgrade() {
                swap_texts(&ui);
            }
        });
        header.pack_start(button);
    }

    if config.retry_button {
        let retry_button = gtk::Button::builder()
            .child(
                &adw::ButtonContent::builder()
                    .icon_name("view-refresh-symbolic")
                    .label("重新翻译")
                    .build(),
            )
            .tooltip_text(if ui.source_view.is_some() {
                "按当前原文重新翻译 (Ctrl+Enter)"
            } else {
                "重新请求翻译"
            })
            .build();
        retry_button.add_css_class("pill");
        let ui_weak = Rc::downgrade(&ui);
        retry_button.connect_clicked(move |_| {
            if let Some(ui) = ui_weak.upgrade() {
                start_task(&ui);
            }
        });
        footer.prepend(&retry_button);
    }

    if let Some(source_view) = &ui.source_view {
        // Ctrl+Enter 在编辑原文后主动发起翻译。
        let submit_key = gtk::EventControllerKey::new();
        let ui_weak = Rc::downgrade(&ui);
        submit_key.connect_key_pressed(move |_, key, _, state| {
            let is_enter = matches!(key, gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter);
            if is_enter && state.contains(gtk::gdk::ModifierType::CONTROL_MASK) {
                if let Some(ui) = ui_weak.upgrade() {
                    start_task(&ui);
                }
                return gtk::glib::Propagation::Stop;
            }
            gtk::glib::Propagation::Proceed
        });
        source_view.add_controller(submit_key);
    }

    // 回调里只持有弱引用，由窗口负责保活 ResultUi；销毁时顺便作废在途请求。
    let keep_alive = ui.clone();
    window.connect_destroy(move |_| {
        keep_alive.generation.set(keep_alive.generation.get() + 1);
    });

    start_task(&ui);

    let key_controller = gtk::EventControllerKey::new();
    let window_for_key = window.clone();
    key_controller.connect_key_pressed(move |_, key, _, _| {
        if key == gtk::gdk::Key::Escape {
            window_for_key.close();
            return gtk::glib::Propagation::Stop;
        }
        gtk::glib::Propagation::Proceed
    });
    window.add_controller(key_controller);

    window.present();
    window
}

/// 以当前左侧原文和语言下拉框的选择发起一次翻译；新请求会作废在途请求。
fn start_task(ui: &Rc<ResultUi>) {
    if ui.swapping.get() {
        return;
    }

    let source = ui.source_view.as_ref().map(buffer_text).unwrap_or_default();
    if ui.source_view.is_some() && source.trim().is_empty() {
        ui.toast_overlay
            .add_toast(adw::Toast::new("原文为空，无法翻译"));
        return;
    }
    let target = ui
        .dropdown
        .as_ref()
        .and_then(|dropdown| target_for_index(dropdown.selected() as usize));

    let request_id = ui.generation.get() + 1;
    ui.generation.set(request_id);
    ui.stack.set_visible_child_name("loading");

    let task = ui.task.clone();
    let (sender, receiver) = async_channel::bounded::<Result<String, String>>(1);
    std::thread::spawn(move || {
        let outcome = task(source, target).map_err(|err| err.to_string());
        let _ = sender.send_blocking(outcome);
    });

    let ui = ui.clone();
    gtk::glib::spawn_future_local(async move {
        let Ok(outcome) = receiver.recv().await else {
            return;
        };
        if ui.generation.get() != request_id {
            return;
        }
        match outcome {
            Ok(text) => {
                ui.result_view.buffer().set_text(&text);
                ui.stack.set_visible_child_name("result");
            }
            Err(message) => {
                if let Some(old) = ui.stack.child_by_name("error") {
                    ui.stack.remove(&old);
                }
                let status = adw::StatusPage::builder()
                    .icon_name("dialog-error-symbolic")
                    .title(ui.error_title)
                    .description(&message)
                    .vexpand(true)
                    .build();
                ui.stack.add_named(&status, Some("error"));
                ui.stack.set_visible_child_name("error");
            }
        }
    });
}

/// 互换原文与译文：两侧文字淡出，在最暗处交换内容，再淡入。
fn swap_texts(ui: &Rc<ResultUi>) {
    let Some(source_view) = ui.source_view.clone() else {
        return;
    };
    if ui.swapping.get() {
        return;
    }
    if ui.stack.visible_child_name().as_deref() != Some("result") {
        ui.toast_overlay
            .add_toast(adw::Toast::new("暂无译文可互换"));
        return;
    }
    let source = buffer_text(&source_view);
    let result = buffer_text(&ui.result_view);
    if result.trim().is_empty() {
        ui.toast_overlay
            .add_toast(adw::Toast::new("暂无译文可互换"));
        return;
    }

    ui.swapping.set(true);
    // 互换后原先的目标语言不再成立，重置为“自动”，避免下次翻译回到同一语言。
    if let Some(dropdown) = &ui.dropdown
        && dropdown.selected() != 0
    {
        ui.suppress_lang_notify.set(true);
        dropdown.set_selected(0);
        ui.suppress_lang_notify.set(false);
    }

    let swapped = Cell::new(false);
    let result_view = ui.result_view.clone();
    let source_for_anim = source_view.clone();
    let target = adw::CallbackAnimationTarget::new(move |progress| {
        // 0 → 0.5 淡出，0.5 → 1 淡入，中点处交换文字。
        if progress >= 0.5 && !swapped.replace(true) {
            source_for_anim.buffer().set_text(&result);
            result_view.buffer().set_text(&source);
        }
        let opacity = (1.0 - 2.0 * progress).abs();
        source_for_anim.set_opacity(opacity);
        result_view.set_opacity(opacity);
    });

    let animation = adw::TimedAnimation::builder()
        .widget(&source_view)
        .value_from(0.0)
        .value_to(1.0)
        .duration(FADE_DURATION_MS * 2)
        .easing(adw::Easing::Linear)
        .target(&target)
        .build();

    let ui_for_done = ui.clone();
    animation.connect_done(move |_| {
        if let Some(view) = &ui_for_done.source_view {
            view.set_opacity(1.0);
        }
        ui_for_done.result_view.set_opacity(1.0);
        ui_for_done.swapping.set(false);
    });
    animation.play();
}

fn build_left_pane(config: &ResultWindowConfig) -> (gtk::Box, Option<gtk::TextView>) {
    let mut editable_source = None;
    let pane = gtk::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(12)
        .build();
    pane.add_css_class("ncaptura-preview-pane");

    let label = gtk::Label::builder()
        .label(config.left_label)
        .halign(Align::Start)
        .margin_top(24)
        .margin_start(24)
        .build();
    label.add_css_class("dim-label");
    label.add_css_class("caption");
    pane.append(&label);

    match &config.left {
        LeftPane::Screenshot(path) => {
            let picture = gtk::Picture::for_filename(path);
            picture.set_content_fit(gtk::ContentFit::Contain);
            picture.set_can_shrink(true);
            picture.set_hexpand(true);
            picture.set_vexpand(true);
            picture.set_margin_start(24);
            picture.set_margin_end(24);
            picture.set_margin_bottom(24);
            pane.append(&picture);
        }
        LeftPane::SourceText(text) => {
            let source_view = gtk::TextView::builder()
                .editable(true)
                .wrap_mode(gtk::WrapMode::WordChar)
                .pixels_below_lines(8)
                .top_margin(12)
                .bottom_margin(24)
                .left_margin(24)
                .right_margin(24)
                .build();
            source_view.buffer().set_text(text);

            let scroller = gtk::ScrolledWindow::builder()
                .child(&source_view)
                .hexpand(true)
                .vexpand(true)
                .build();
            pane.append(&scroller);
            editable_source = Some(source_view);
        }
    }

    (pane, editable_source)
}

fn build_right_pane<G>(
    config: &ResultWindowConfig,
    toast_overlay: &adw::ToastOverlay,
    app: &adw::Application,
    on_translate: Option<G>,
) -> (gtk::Box, gtk::Stack, gtk::TextView, gtk::Box)
where
    G: Fn(&adw::Application, String) + 'static,
{
    let pane = gtk::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(12)
        .build();

    let label = gtk::Label::builder()
        .label(config.right_label)
        .halign(Align::Start)
        .margin_top(24)
        .margin_start(28)
        .build();
    label.add_css_class("dim-label");
    label.add_css_class("caption");
    pane.append(&label);

    let stack = gtk::Stack::builder()
        .hexpand(true)
        .vexpand(true)
        .transition_type(gtk::StackTransitionType::Crossfade)
        .build();

    let loading = gtk::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(12)
        .valign(Align::Center)
        .halign(Align::Center)
        .build();
    let spinner = gtk::Spinner::builder()
        .width_request(32)
        .height_request(32)
        .build();
    spinner.start();
    let loading_label = gtk::Label::new(Some(config.loading_label));
    loading_label.add_css_class("dim-label");
    loading.append(&spinner);
    loading.append(&loading_label);
    stack.add_named(&loading, Some("loading"));

    let text_view = gtk::TextView::builder()
        .editable(true)
        .wrap_mode(gtk::WrapMode::WordChar)
        .pixels_below_lines(8)
        .top_margin(12)
        .bottom_margin(12)
        .left_margin(28)
        .right_margin(28)
        .build();
    let scroller = gtk::ScrolledWindow::builder()
        .child(&text_view)
        .hexpand(true)
        .vexpand(true)
        .build();
    stack.add_named(&scroller, Some("result"));
    stack.set_visible_child_name("loading");
    pane.append(&stack);

    let footer = gtk::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .halign(Align::End)
        .margin_bottom(24)
        .margin_end(24)
        .build();

    if config.translate_button
        && let Some(handler) = on_translate
    {
        let translate_button = gtk::Button::builder()
            .child(
                &adw::ButtonContent::builder()
                    .icon_name("preferences-desktop-locale-symbolic")
                    .label("翻译")
                    .build(),
            )
            .build();
        translate_button.add_css_class("pill");
        let app_for_translate = app.clone();
        let text_for_translate = text_view.clone();
        let overlay = toast_overlay.clone();
        translate_button.connect_clicked(move |_| {
            let buffer = text_for_translate.buffer();
            let text = buffer
                .text(&buffer.start_iter(), &buffer.end_iter(), false)
                .trim()
                .to_string();
            if text.is_empty() {
                overlay.add_toast(adw::Toast::new("暂无可翻译的内容"));
                return;
            }
            handler(&app_for_translate, text);
        });
        footer.append(&translate_button);
    }

    let strip_button = gtk::Button::builder()
        .child(
            &adw::ButtonContent::builder()
                .icon_name("format-justify-left-symbolic")
                .label("删除换行")
                .build(),
        )
        .build();
    strip_button.add_css_class("pill");
    let strip_target = text_view.clone();
    strip_button.connect_clicked(move |_| {
        let buffer = strip_target.buffer();
        let text = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .to_string();
        buffer.set_text(&remove_line_breaks(&text));
    });
    footer.append(&strip_button);

    let copy_button = gtk::Button::builder()
        .child(
            &adw::ButtonContent::builder()
                .icon_name("edit-copy-symbolic")
                .label("复制")
                .build(),
        )
        .build();
    copy_button.add_css_class("pill");
    copy_button.add_css_class("suggested-action");
    let copy_source = text_view.clone();
    let overlay = toast_overlay.clone();
    copy_button.connect_clicked(move |_| {
        let buffer = copy_source.buffer();
        let text = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .to_string();
        let toast = match copy_text(&text) {
            Ok(()) => adw::Toast::new("已复制到剪贴板"),
            Err(err) => adw::Toast::new(&format!("复制失败: {err}")),
        };
        overlay.add_toast(toast);
    });
    footer.append(&copy_button);

    pane.append(&footer);

    (pane, stack, text_view, footer)
}

pub fn apply_result_window_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_data(
        "
        .ncaptura-preview-pane {
            background-color: alpha(@window_fg_color, 0.045);
        }

        button.ncaptura-swap-button {
            font-size: 16px;
            min-width: 32px;
            padding: 0 8px;
        }

        window.ncaptura-tool-window {
            background-color: @window_bg_color;
            border-radius: 18px;
        }
        ",
    );

    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{WindowLayout, target_for_index};

    #[test]
    fn auto_language_maps_to_no_explicit_target() {
        assert_eq!(target_for_index(0), None);
        assert_eq!(target_for_index(1).as_deref(), Some("zh-CN"));
        assert_eq!(target_for_index(999), None);
    }

    #[test]
    fn landscape_monitor_uses_horizontal_layout() {
        let layout = WindowLayout::for_monitor(Some((2560, 1440)));
        assert!(!layout.vertical);
        assert_eq!((layout.width, layout.height), (1060, 620));
        assert_eq!(layout.pane_position, 1060 * 44 / 100);
    }

    #[test]
    fn portrait_monitor_uses_vertical_layout_and_caps_width() {
        let layout = WindowLayout::for_monitor(Some((1080, 1920)));
        assert!(layout.vertical);
        assert_eq!(layout.width, 860);
        assert_eq!(layout.height, 1100);
        assert_eq!(layout.pane_position, 1100 * 42 / 100);
    }

    #[test]
    fn narrow_portrait_monitor_shrinks_to_usable_width() {
        let layout = WindowLayout::for_monitor(Some((720, 1280)));
        assert!(layout.vertical);
        assert_eq!(layout.width, 720 * 92 / 100);
        assert!(layout.width < 720);
    }

    #[test]
    fn small_landscape_monitor_stays_within_screen() {
        let layout = WindowLayout::for_monitor(Some((900, 700)));
        assert!(!layout.vertical);
        assert!(layout.width <= 900 * 92 / 100);
        assert!(layout.height <= 700 * 85 / 100);
    }

    #[test]
    fn missing_monitor_falls_back_to_landscape_default() {
        let layout = WindowLayout::for_monitor(None);
        assert!(!layout.vertical);
        assert_eq!((layout.width, layout.height), (1060, 620));
    }
}
