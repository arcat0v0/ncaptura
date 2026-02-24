use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{
    Align, Box as GtkBox, Button, Image, Label, ListBox, Orientation, SelectionMode, Switch,
    ToggleButton,
};

use crate::capture::{self, CaptureTarget, RecordingSession};
use crate::ui::recording_hud::show_recording_hud;

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
    let is_record_mode = Rc::new(RefCell::new(false));
    let recording_session: Rc<RefCell<Option<RecordingSession>>> = Rc::new(RefCell::new(None));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Take Screenshot")
        .resizable(false)
        .default_width(408)
        .default_height(312)
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

    let content = GtkBox::new(Orientation::Vertical, 16);
    content.set_halign(Align::Center);
    content.set_valign(Align::Start);
    content.set_margin_top(16);
    content.set_margin_bottom(16);
    content.set_margin_start(24);
    content.set_margin_end(24);

    let mode_stack = gtk::Stack::new();
    mode_stack.add_titled(
        &gtk::Box::new(Orientation::Vertical, 0),
        Some("screenshot"),
        "Screenshot",
    );
    mode_stack.add_titled(
        &gtk::Box::new(Orientation::Vertical, 0),
        Some("recording"),
        "Recording",
    );
    mode_stack.set_visible_child_name("screenshot");

    let mode_tabs = gtk::StackSwitcher::new();
    mode_tabs.set_stack(Some(&mode_stack));
    mode_tabs.set_halign(Align::Center);

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

    let audio_row = adw::ActionRow::builder().title("Record Audio").build();
    let audio_switch = Switch::new();
    audio_switch.set_valign(Align::Center);
    audio_row.add_suffix(&audio_switch);
    audio_row.set_visible(false);
    options_list.append(&audio_row);

    let delay_row = adw::ActionRow::builder().title("Delay in Seconds").build();
    let delay_spin = gtk::SpinButton::with_range(0.0, 99.0, 1.0);
    delay_spin.set_valign(Align::Center);
    delay_spin.set_numeric(true);
    delay_spin.set_snap_to_ticks(true);
    delay_row.add_suffix(&delay_spin);
    options_list.append(&delay_row);

    content.append(&mode_tabs);
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
        let is_record_mode = is_record_mode.clone();
        let recording_session = recording_session.clone();
        let take_screenshot_button = take_screenshot_button.clone();
        let pointer_row = pointer_row.clone();
        let audio_row = audio_row.clone();
        mode_stack.connect_visible_child_name_notify(move |stack| {
            let recording_mode = stack.visible_child_name().as_deref() == Some("recording");
            *is_record_mode.borrow_mut() = recording_mode;
            pointer_row.set_sensitive(!recording_mode);
            audio_row.set_visible(recording_mode);
            if recording_mode {
                if recording_session.borrow().is_some() {
                    take_screenshot_button.set_label("Stop Recording");
                } else {
                    take_screenshot_button.set_label("Start Recording");
                }
            } else {
                take_screenshot_button.set_label("Take Screenshot");
            }
        });
    }

    {
        let app = app.clone();
        let selected_mode = selected_mode.clone();
        let show_pointer = show_pointer.clone();
        let delay_seconds = delay_seconds.clone();
        let is_record_mode = is_record_mode.clone();
        let audio_switch = audio_switch.clone();
        let recording_session = recording_session.clone();
        let take_screenshot_button_handle = take_screenshot_button.clone();
        let take_screenshot_button = take_screenshot_button.clone();
        let mode_stack = mode_stack.clone();
        let window_handle = window.clone();
        take_screenshot_button_handle.connect_clicked(move |_| {
            if *is_record_mode.borrow() {
                if recording_session.borrow().is_some() {
                    return;
                }

                let target = match *selected_mode.borrow() {
                    CaptureMode::Screen => CaptureTarget::Fullscreen,
                    CaptureMode::Window => CaptureTarget::Region,
                    CaptureMode::Selection => CaptureTarget::Region,
                };

                match capture::start_recording(target, audio_switch.is_active()) {
                    Ok(session) => {
                        *recording_session.borrow_mut() = Some(session);
                        take_screenshot_button.set_label("Stop Recording");
                        window_handle.set_visible(false);
                        show_recording_hud(
                            &app,
                            &window_handle,
                            &mode_stack,
                            &take_screenshot_button,
                            &recording_session,
                        );
                    }
                    Err(err) => eprintln!("开始录屏失败: {err}"),
                }
                return;
            }

            let result = InteractiveDialogResult {
                mode: *selected_mode.borrow(),
                show_pointer: *show_pointer.borrow(),
                delay_seconds: *delay_seconds.borrow(),
            };
            window_handle.destroy();
            on_take(result);
        });
    }

    {
        let recording_session = recording_session.clone();
        window.connect_close_request(move |_| {
            if let Some(session) = recording_session.borrow_mut().take() {
                let _ = capture::stop_recording(session);
            }
            gtk::glib::Propagation::Proceed
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
