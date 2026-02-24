use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use adw::prelude::*;
use gtk::{Align, Box as GtkBox, Button, CssProvider, Label, Orientation};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use nix::errno::Errno;
use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;

use crate::capture::{self, CliRecordingState};

pub fn run_cli_recording_hud(initial_state: CliRecordingState) {
    let app = adw::Application::builder()
        .application_id("io.ncaptura.app.cli-recording-hud")
        .build();

    app.connect_activate(move |app| {
        build_cli_recording_hud(app, initial_state.clone());
    });
    let _ = app.run_with_args(&["ncaptura-cli-recording-hud"]);
}

fn build_cli_recording_hud(app: &adw::Application, initial_state: CliRecordingState) {
    apply_cli_recording_hud_css();

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
        hud.set_namespace(Some("ncaptura-cli-recording-hud"));
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

    let recording_pid = Rc::new(Cell::new(initial_state.pid));
    let started_at = Instant::now();
    let paused_since: Rc<RefCell<Option<Instant>>> = Rc::new(RefCell::new(None));
    let paused_total = Rc::new(RefCell::new(Duration::ZERO));
    let blinking_visible = Rc::new(RefCell::new(true));
    let closing = Rc::new(Cell::new(false));

    let blink_source: Rc<RefCell<Option<gtk::glib::SourceId>>> = Rc::new(RefCell::new(None));
    let timer_source: Rc<RefCell<Option<gtk::glib::SourceId>>> = Rc::new(RefCell::new(None));
    let monitor_source: Rc<RefCell<Option<gtk::glib::SourceId>>> = Rc::new(RefCell::new(None));

    let finalize: Rc<dyn Fn(bool)> = Rc::new({
        let app = app.clone();
        let hud = hud.clone();
        let closing = closing.clone();
        let blink_source = blink_source.clone();
        let timer_source = timer_source.clone();
        let monitor_source = monitor_source.clone();
        move |request_stop| {
            if closing.replace(true) {
                return;
            }

            if request_stop {
                match capture::stop_recording_detached() {
                    Ok(path) => eprintln!("录屏已停止，文件保存为: {}", path.display()),
                    Err(err) => eprintln!("停止录屏失败: {err}"),
                }
            }

            if let Some(source) = blink_source.borrow_mut().take() {
                source.remove();
            }
            if let Some(source) = timer_source.borrow_mut().take() {
                source.remove();
            }
            if let Some(source) = monitor_source.borrow_mut().take() {
                source.remove();
            }

            hud.close();
            app.quit();
        }
    });

    {
        let timer_label = timer_label.clone();
        let paused_since = paused_since.clone();
        let paused_total = paused_total.clone();
        let source = gtk::glib::timeout_add_local(Duration::from_secs(1), move || {
            let now = Instant::now();
            let extra_paused = paused_since
                .borrow()
                .map(|start| now.duration_since(start))
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
        let recording_pid = recording_pid.clone();
        let paused_since = paused_since.clone();
        let paused_total = paused_total.clone();
        let indicator = indicator.clone();
        let pause_button_handle = pause_button.clone();
        let pause_button = pause_button.clone();
        let finalize = finalize.clone();
        pause_button_handle.connect_clicked(move |_| {
            let pid = recording_pid.get();
            let process_id = Pid::from_raw(pid as i32);

            if paused_since.borrow().is_none() {
                match kill(process_id, Signal::SIGSTOP) {
                    Ok(_) => {
                        *paused_since.borrow_mut() = Some(Instant::now());
                        indicator.add_css_class("paused");
                        indicator.set_opacity(1.0);
                        pause_button.set_icon_name("media-playback-start-symbolic");
                    }
                    Err(err) if err == Errno::ESRCH => finalize(false),
                    Err(err) => eprintln!("暂停录屏失败: {err}"),
                }
                return;
            }

            match kill(process_id, Signal::SIGCONT) {
                Ok(_) => {
                    if let Some(start) = paused_since.borrow_mut().take() {
                        *paused_total.borrow_mut() += Instant::now().duration_since(start);
                    }
                    indicator.remove_css_class("paused");
                    pause_button.set_icon_name("media-playback-pause-symbolic");
                }
                Err(err) if err == Errno::ESRCH => finalize(false),
                Err(err) => eprintln!("恢复录屏失败: {err}"),
            }
        });
    }

    {
        let finalize = finalize.clone();
        stop_button.connect_clicked(move |_| finalize(true));
    }

    {
        let recording_pid = recording_pid.clone();
        let finalize = finalize.clone();
        let source = gtk::glib::timeout_add_local(Duration::from_millis(500), move || {
            match capture::current_cli_recording_state() {
                Ok(state) => {
                    recording_pid.set(state.pid);
                    if process_is_running(state.pid) {
                        gtk::glib::ControlFlow::Continue
                    } else {
                        finalize(false);
                        gtk::glib::ControlFlow::Break
                    }
                }
                Err(_) => {
                    finalize(false);
                    gtk::glib::ControlFlow::Break
                }
            }
        });
        *monitor_source.borrow_mut() = Some(source);
    }

    {
        let finalize = finalize.clone();
        hud.connect_close_request(move |_| {
            finalize(true);
            gtk::glib::Propagation::Stop
        });
    }

    hud.present();
}

fn process_is_running(pid: u32) -> bool {
    let process_id = Pid::from_raw(pid as i32);
    match kill(process_id, None) {
        Ok(_) => true,
        Err(err) => err != Errno::ESRCH,
    }
}

fn apply_cli_recording_hud_css() {
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
