use adw::prelude::*;
use gtk::{Align, Orientation};

pub fn build_translate_input_window<F>(
    app: &adw::Application,
    on_submit: F,
) -> adw::ApplicationWindow
where
    F: Fn(&adw::Application, String) + 'static,
{
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("输入翻译")
        .default_width(560)
        .default_height(-1)
        .build();

    let toolbar_view = adw::ToolbarView::new();
    let header = adw::HeaderBar::builder()
        .title_widget(&adw::WindowTitle::new("输入翻译", ""))
        .show_start_title_buttons(false)
        .decoration_layout(":close")
        .build();
    toolbar_view.add_top_bar(&header);

    let content = gtk::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(12)
        .margin_top(18)
        .margin_bottom(18)
        .margin_start(18)
        .margin_end(18)
        .build();

    let entry = gtk::Entry::builder()
        .placeholder_text("输入待翻译文本，回车翻译")
        .activates_default(true)
        .hexpand(true)
        .build();
    content.append(&entry);

    let hint = gtk::Label::builder()
        .label("回车翻译，Esc 关闭")
        .halign(Align::Start)
        .build();
    hint.add_css_class("dim-label");
    hint.add_css_class("caption");
    content.append(&hint);

    let clamp = adw::Clamp::builder()
        .child(&content)
        .maximum_size(560)
        .build();
    toolbar_view.set_content(Some(&clamp));
    window.set_content(Some(&toolbar_view));

    let app_for_entry = app.clone();
    let window_for_entry = window.clone();
    entry.connect_activate(move |entry| {
        let text = entry.text().trim().to_string();
        if text.is_empty() {
            return;
        }
        on_submit(&app_for_entry, text);
        window_for_entry.close();
    });

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
    entry.grab_focus();
    window
}
