use std::path::PathBuf;

use adw::prelude::*;
use gtk::{Align, Orientation};
use gtk4_layer_shell::{KeyboardMode, Layer, LayerShell};

use crate::capture::copy_text_to_clipboard;
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

type SharedTask = std::sync::Arc<dyn Fn(Option<String>) -> anyhow::Result<String> + Send + Sync>;

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
    let focused = crate::capture::focused_output_name().ok();

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
    F: Fn(Option<String>) -> anyhow::Result<String> + Send + Sync + 'static,
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

    if gtk4_layer_shell::is_supported() {
        window.init_layer_shell();
        window.set_layer(Layer::Top);
        window.set_keyboard_mode(KeyboardMode::Exclusive);
        window.set_namespace(Some(config.namespace));
    }
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

    paned.set_start_child(Some(&build_left_pane(&config)));
    let (right_pane, stack, text_view) =
        build_right_pane(&config, &toast_overlay, app, on_translate);
    paned.set_end_child(Some(&right_pane));

    toolbar_view.set_content(Some(&paned));
    toast_overlay.set_child(Some(&toolbar_view));
    window.set_content(Some(&toast_overlay));

    let task: SharedTask = std::sync::Arc::new(task);

    if config.language_selector {
        let labels: Vec<&str> = TARGET_LANGUAGES.iter().map(|(_, label)| *label).collect();
        let dropdown = gtk::DropDown::from_strings(&labels);
        dropdown.set_tooltip_text(Some("目标语言"));
        let stack_for_lang = stack.clone();
        let text_for_lang = text_view.clone();
        let task_for_lang = task.clone();
        let error_title = config.error_title;
        dropdown.connect_selected_notify(move |dropdown| {
            let index = dropdown.selected() as usize;
            let Some((code, _)) = TARGET_LANGUAGES.get(index) else {
                return;
            };
            let target = if code.is_empty() {
                None
            } else {
                Some(code.to_string())
            };
            start_task(
                &stack_for_lang,
                &text_for_lang,
                error_title,
                task_for_lang.clone(),
                target,
            );
        });
        header.pack_end(&dropdown);
    }

    start_task(&stack, &text_view, config.error_title, task, None);

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

fn start_task(
    stack: &gtk::Stack,
    text_view: &gtk::TextView,
    error_title: &'static str,
    task: SharedTask,
    target: Option<String>,
) {
    stack.set_visible_child_name("loading");

    let (sender, receiver) = async_channel::bounded::<Result<String, String>>(1);
    std::thread::spawn(move || {
        let outcome = task(target).map_err(|err| err.to_string());
        let _ = sender.send_blocking(outcome);
    });

    let stack = stack.clone();
    let text_view = text_view.clone();
    gtk::glib::spawn_future_local(async move {
        if let Ok(outcome) = receiver.recv().await {
            match outcome {
                Ok(text) => {
                    text_view.buffer().set_text(&text);
                    stack.set_visible_child_name("result");
                }
                Err(message) => {
                    if let Some(old) = stack.child_by_name("error") {
                        stack.remove(&old);
                    }
                    let status = adw::StatusPage::builder()
                        .icon_name("dialog-error-symbolic")
                        .title(error_title)
                        .description(&message)
                        .vexpand(true)
                        .build();
                    stack.add_named(&status, Some("error"));
                    stack.set_visible_child_name("error");
                }
            }
        }
    });
}

fn build_left_pane(config: &ResultWindowConfig) -> gtk::Box {
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
                .editable(false)
                .cursor_visible(false)
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
        }
    }

    pane
}

fn build_right_pane<G>(
    config: &ResultWindowConfig,
    toast_overlay: &adw::ToastOverlay,
    app: &adw::Application,
    on_translate: Option<G>,
) -> (gtk::Box, gtk::Stack, gtk::TextView)
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
        let toast = match copy_text_to_clipboard(&text) {
            Ok(()) => adw::Toast::new("已复制到剪贴板"),
            Err(err) => adw::Toast::new(&format!("复制失败: {err}")),
        };
        overlay.add_toast(toast);
    });
    footer.append(&copy_button);

    pane.append(&footer);

    (pane, stack, text_view)
}

pub fn apply_result_window_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_data(
        "
        .ncaptura-preview-pane {
            background-color: alpha(@window_fg_color, 0.045);
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
    use super::WindowLayout;

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
