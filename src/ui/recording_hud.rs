use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use adw::prelude::*;
use gtk::{Align, Box as GtkBox, Button, CssProvider, Label, Orientation};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::capture::{self, RecordingSession};

pub(super) fn show_recording_hud(
    app: &adw::Application,
    main_window: &adw::ApplicationWindow,
    mode_stack: &gtk::Stack,
    action_button: &Button,
    recording_session: &Rc<RefCell<Option<RecordingSession>>>,
) {
    apply_recording_hud_css();

    let hud = adw::ApplicationWindow::builder()
        .application(app)
        .title("Recording")
        .default_width(300)
        .default_height(50)
        .resizable(false)
        .build();
    hud.set_decorated(false);
    hud.set_size_request(300, 50);
    hud.add_css_class("recording-hud");

    if gtk4_layer_shell::is_supported() {
        hud.init_layer_shell();
        hud.set_layer(Layer::Overlay);
        hud.set_anchor(Edge::Top, true);
        hud.set_anchor(Edge::Right, true);
        hud.set_margin(Edge::Top, 12);
        hud.set_margin(Edge::Right, 12);
        hud.set_keyboard_mode(KeyboardMode::OnDemand);
        hud.set_namespace(Some("ncaptura-recording-hud"));
    }

    let row = GtkBox::new(Orientation::Horizontal, 10);
    row.set_margin_top(4);
    row.set_margin_bottom(4);
    row.set_margin_start(12);
    row.set_margin_end(12);
    row.set_halign(Align::Fill);

    let indicator = Label::new(Some("●"));
    indicator.add_css_class("recording-indicator");

    let timer_label = Label::new(Some("00:00:00"));
    timer_label.add_css_class("title-4");
    timer_label.set_hexpand(true);
    timer_label.set_halign(Align::Start);

    let pause_button = Button::builder()
        .icon_name("media-playback-pause-symbolic")
        .tooltip_text("Pause/Resume")
        .build();
    pause_button.add_css_class("pause-record-btn");

    let stop_button = Button::builder()
        .icon_name("media-record-symbolic")
        .tooltip_text("Stop Recording")
        .build();
    stop_button.add_css_class("stop-record-btn");

    row.append(&indicator);
    row.append(&timer_label);
    row.append(&pause_button);
    row.append(&stop_button);
    hud.set_content(Some(&row));

    let started_at = Instant::now();
    let paused_since: Rc<RefCell<Option<Instant>>> = Rc::new(RefCell::new(None));
    let paused_total = Rc::new(RefCell::new(Duration::ZERO));
    let blinking_visible = Rc::new(RefCell::new(true));
    let blink_source: Rc<RefCell<Option<gtk::glib::SourceId>>> = Rc::new(RefCell::new(None));
    let timer_source: Rc<RefCell<Option<gtk::glib::SourceId>>> = Rc::new(RefCell::new(None));

    {
        let timer_label = timer_label.clone();
        let paused_since = paused_since.clone();
        let paused_total = paused_total.clone();
        let source = gtk::glib::timeout_add_local(Duration::from_secs(1), move || {
            let now = Instant::now();
            let extra_paused = paused_since
                .borrow()
                .map(|s| now.duration_since(s))
                .unwrap_or(Duration::ZERO);
            let elapsed = now.duration_since(started_at) - *paused_total.borrow() - extra_paused;
            let seconds = elapsed.as_secs();
            let h = seconds / 3600;
            let m = (seconds % 3600) / 60;
            let s = seconds % 60;
            timer_label.set_text(&format!("{h:02}:{m:02}:{s:02}"));
            gtk::glib::ControlFlow::Continue
        });
        *timer_source.borrow_mut() = Some(source);
    }

    {
        let indicator = indicator.clone();
        let paused_since = paused_since.clone();
        let blinking_visible = blinking_visible.clone();
        let source = gtk::glib::timeout_add_local(Duration::from_millis(500), move || {
            if paused_since.borrow().is_some() {
                indicator.set_opacity(1.0);
                return gtk::glib::ControlFlow::Continue;
            }
            let mut visible = blinking_visible.borrow_mut();
            *visible = !*visible;
            indicator.set_opacity(if *visible { 1.0 } else { 0.2 });
            gtk::glib::ControlFlow::Continue
        });
        *blink_source.borrow_mut() = Some(source);
    }

    {
        let recording_session = recording_session.clone();
        let paused_since = paused_since.clone();
        let paused_total = paused_total.clone();
        let indicator = indicator.clone();
        let pause_button_handle = pause_button.clone();
        let pause_button = pause_button.clone();
        pause_button_handle.connect_clicked(move |_| {
            let mut session_ref = recording_session.borrow_mut();
            let Some(session) = session_ref.as_mut() else {
                return;
            };
            match capture::toggle_recording_pause(session) {
                Ok(true) => {
                    *paused_since.borrow_mut() = Some(Instant::now());
                    indicator.add_css_class("paused");
                    indicator.set_opacity(1.0);
                    pause_button.set_icon_name("media-playback-start-symbolic");
                }
                Ok(false) => {
                    if let Some(since) = paused_since.borrow_mut().take() {
                        *paused_total.borrow_mut() += Instant::now().duration_since(since);
                    }
                    indicator.remove_css_class("paused");
                    pause_button.set_icon_name("media-playback-pause-symbolic");
                }
                Err(err) => eprintln!("切换暂停状态失败: {err}"),
            }
        });
    }

    {
        let hud = hud.clone();
        let main_window = main_window.clone();
        let mode_stack = mode_stack.clone();
        let action_button = action_button.clone();
        let recording_session = recording_session.clone();
        let blink_source = blink_source.clone();
        let timer_source = timer_source.clone();
        stop_button.connect_clicked(move |_| {
            if let Some(session) = recording_session.borrow_mut().take() {
                match capture::stop_recording(session) {
                    Ok(path) => eprintln!("录屏已保存: {}", path.display()),
                    Err(err) => eprintln!("停止录屏失败: {err}"),
                }
            }
            if let Some(source) = blink_source.borrow_mut().take() {
                source.remove();
            }
            if let Some(source) = timer_source.borrow_mut().take() {
                source.remove();
            }
            hud.destroy();
            mode_stack.set_visible_child_name("recording");
            action_button.set_label("Start Recording");
            main_window.present();
        });
    }

    {
        let recording_session = recording_session.clone();
        let blink_source = blink_source.clone();
        let timer_source = timer_source.clone();
        let main_window = main_window.clone();
        let mode_stack = mode_stack.clone();
        let action_button = action_button.clone();
        hud.connect_close_request(move |_| {
            if let Some(session) = recording_session.borrow_mut().take() {
                let _ = capture::stop_recording(session);
            }
            if let Some(source) = blink_source.borrow_mut().take() {
                source.remove();
            }
            if let Some(source) = timer_source.borrow_mut().take() {
                source.remove();
            }
            mode_stack.set_visible_child_name("recording");
            action_button.set_label("Start Recording");
            main_window.present();
            gtk::glib::Propagation::Proceed
        });
    }

    hud.present();
}

fn apply_recording_hud_css() {
    let provider = CssProvider::new();
    provider.load_from_data(
        "
        window.recording-hud {
            background: rgba(30, 30, 30, 0.88);
            border-radius: 14px;
        }

        window.recording-hud label.recording-indicator {
            color: #e53935;
            font-size: 10px;
            font-weight: 700;
        }

        window.recording-hud label.recording-indicator.paused {
            color: #f4b400;
        }

        window.recording-hud button.stop-record-btn {
            min-width: 34px;
            min-height: 34px;
            border-radius: 999px;
            background: #d32f2f;
            color: white;
        }

        window.recording-hud button.pause-record-btn {
            min-width: 34px;
            min-height: 34px;
            border-radius: 999px;
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
