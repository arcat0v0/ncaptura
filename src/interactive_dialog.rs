use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{
    Align, Box as GtkBox, Button, Image, Label, ListBox, Orientation, SelectionMode, Switch,
    ToggleButton,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CaptureMode {
    Screen,
    Window,
    Selection,
}

#[derive(Clone, Copy, Debug)]
pub struct InteractiveDialogResult {
    pub mode: CaptureMode,
    pub show_pointer: bool,
    pub delay_seconds: u32,
}

pub fn build_interactive_dialog(
    app: &adw::Application,
    on_take: impl Fn(InteractiveDialogResult) + 'static,
) -> adw::ApplicationWindow {
    let selected_mode = Rc::new(RefCell::new(CaptureMode::Selection));
    let show_pointer = Rc::new(RefCell::new(false));
    let delay_seconds = Rc::new(RefCell::new(0_u32));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Take Screenshot")
        .resizable(false)
        .default_width(408)
        .default_height(356)
        .build();

    let root = GtkBox::new(Orientation::Vertical, 0);

    let header_bar = adw::HeaderBar::new();

    let take_screenshot_button = Button::with_label("Take Screenshot");
    take_screenshot_button.add_css_class("suggested-action");

    let menu_button = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .build();

    header_bar.pack_start(&take_screenshot_button);
    header_bar.pack_end(&menu_button);

    let content = GtkBox::new(Orientation::Vertical, 24);
    content.set_halign(Align::Center);
    content.set_valign(Align::Center);
    content.set_margin_top(24);
    content.set_margin_bottom(24);
    content.set_margin_start(24);
    content.set_margin_end(24);

    let capture_section = GtkBox::new(Orientation::Vertical, 6);

    let capture_label = Label::new(Some("Capture Area"));
    capture_label.set_halign(Align::Start);
    capture_section.append(&capture_label);

    let mode_row = GtkBox::new(Orientation::Horizontal, 0);
    mode_row.add_css_class("linked");
    mode_row.set_homogeneous(true);

    let screen_button = build_mode_button("video-display-symbolic", "Screen");
    let window_button = build_mode_button("window-new-symbolic", "Window");
    let selection_button = build_mode_button("selection-mode-symbolic", "Selection");

    window_button.set_group(Some(&screen_button));
    selection_button.set_group(Some(&screen_button));
    selection_button.set_active(true);

    mode_row.append(&screen_button);
    mode_row.append(&window_button);
    mode_row.append(&selection_button);
    capture_section.append(&mode_row);

    let options_list = ListBox::new();
    options_list.set_selection_mode(SelectionMode::None);
    options_list.set_width_request(360);
    options_list.add_css_class("boxed-list");

    let pointer_row = adw::ActionRow::builder().title("Show Pointer").build();
    let pointer_switch = Switch::new();
    pointer_switch.set_valign(Align::Center);
    pointer_row.add_suffix(&pointer_switch);
    options_list.append(&pointer_row);

    let delay_row = adw::ActionRow::builder().title("Delay in Seconds").build();
    let delay_spin = gtk::SpinButton::with_range(0.0, 99.0, 1.0);
    delay_spin.set_valign(Align::Center);
    delay_spin.set_numeric(true);
    delay_spin.set_snap_to_ticks(true);
    delay_row.add_suffix(&delay_spin);
    options_list.append(&delay_row);

    content.append(&capture_section);
    content.append(&options_list);

    root.append(&header_bar);
    root.append(&content);
    window.set_content(Some(&root));

    {
        let selected_mode = selected_mode.clone();
        screen_button.connect_toggled(move |button| {
            if button.is_active() {
                *selected_mode.borrow_mut() = CaptureMode::Screen;
            }
        });
    }

    {
        let selected_mode = selected_mode.clone();
        window_button.connect_toggled(move |button| {
            if button.is_active() {
                *selected_mode.borrow_mut() = CaptureMode::Window;
            }
        });
    }

    {
        let selected_mode = selected_mode.clone();
        selection_button.connect_toggled(move |button| {
            if button.is_active() {
                *selected_mode.borrow_mut() = CaptureMode::Selection;
            }
        });
    }

    {
        let show_pointer = show_pointer.clone();
        pointer_switch.connect_active_notify(move |switch| {
            *show_pointer.borrow_mut() = switch.is_active();
        });
    }

    {
        let delay_seconds = delay_seconds.clone();
        delay_spin.connect_value_changed(move |spin| {
            *delay_seconds.borrow_mut() = spin.value_as_int() as u32;
        });
    }

    {
        let selected_mode = selected_mode.clone();
        let show_pointer = show_pointer.clone();
        let delay_seconds = delay_seconds.clone();
        let window_handle = window.clone();
        take_screenshot_button.connect_clicked(move |_| {
            let result = InteractiveDialogResult {
                mode: *selected_mode.borrow(),
                show_pointer: *show_pointer.borrow(),
                delay_seconds: *delay_seconds.borrow(),
            };
            window_handle.destroy();
            on_take(result);
        });
    }

    window.present();
    window
}

fn build_mode_button(icon_name: &str, label_text: &str) -> ToggleButton {
    let button = ToggleButton::new();

    let content = GtkBox::new(Orientation::Vertical, 6);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.set_margin_start(12);
    content.set_margin_end(12);

    let icon = Image::from_icon_name(icon_name);
    icon.set_pixel_size(32);

    let label = Label::new(Some(label_text));

    content.append(&icon);
    content.append(&label);
    button.set_child(Some(&content));

    button
}
