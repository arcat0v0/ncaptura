use std::path::PathBuf;

use adw::prelude::*;
use gtk::{Align, Orientation};

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
    pub left: LeftPane,
}

pub fn build_result_window<F>(
    app: &adw::Application,
    config: ResultWindowConfig,
    task: F,
) -> adw::ApplicationWindow
where
    F: FnOnce() -> anyhow::Result<String> + Send + 'static,
{
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title(config.title)
        .default_width(1060)
        .default_height(620)
        .build();

    let toast_overlay = adw::ToastOverlay::new();
    let toolbar_view = adw::ToolbarView::new();

    let header = adw::HeaderBar::builder()
        .title_widget(&adw::WindowTitle::new(config.title, ""))
        .show_start_title_buttons(false)
        .decoration_layout(":close")
        .build();
    toolbar_view.add_top_bar(&header);

    let paned = gtk::Paned::builder()
        .orientation(Orientation::Horizontal)
        .resize_start_child(false)
        .shrink_start_child(false)
        .resize_end_child(true)
        .shrink_end_child(false)
        .position(470)
        .build();

    paned.set_start_child(Some(&build_left_pane(&config)));
    let (right_pane, stack, text_view) = build_right_pane(&config, &toast_overlay);
    paned.set_end_child(Some(&right_pane));

    toolbar_view.set_content(Some(&paned));
    toast_overlay.set_child(Some(&toolbar_view));
    window.set_content(Some(&toast_overlay));

    let (sender, receiver) = async_channel::bounded::<Result<String, String>>(1);
    std::thread::spawn(move || {
        let outcome = task().map_err(|err| err.to_string());
        let _ = sender.send_blocking(outcome);
    });

    let error_title = config.error_title;
    gtk::glib::spawn_future_local(async move {
        if let Ok(outcome) = receiver.recv().await {
            match outcome {
                Ok(text) => {
                    text_view.buffer().set_text(&text);
                    stack.set_visible_child_name("result");
                }
                Err(message) => {
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

    window.present();
    window
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

fn build_right_pane(
    config: &ResultWindowConfig,
    toast_overlay: &adw::ToastOverlay,
) -> (gtk::Box, gtk::Stack, gtk::TextView) {
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
