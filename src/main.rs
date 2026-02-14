mod capture;

use std::cell::RefCell;
use std::env;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::gdk;
use gtk::prelude::*;
use gtk::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, HeaderBar, Label, Orientation,
    Spinner, ToggleButton,
};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::capture::{CaptureTarget, RecordingSession};

const WINDOW_WIDTH: i32 = 360;
const WINDOW_HEIGHT: i32 = 460;
const WINDOW_MARGIN: i32 = 18;

#[derive(Default)]
struct RecordingUiState {
    session: Option<RecordingSession>,
    started_at: Option<Instant>,
    target: Option<CaptureTarget>,
    with_audio: bool,
    ticker: Option<gtk::glib::SourceId>,
}

fn main() {
    if let Err(code) = handle_cli_if_requested() {
        std::process::exit(code);
    }

    let app = Application::builder()
        .application_id("io.ncaptura.app")
        .build();

    app.connect_activate(build_ui);
    app.run();
}

fn handle_cli_if_requested() -> Result<(), i32> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        return Ok(());
    }

    let result = match parse_cli_command(&args) {
        Ok(command) => run_cli_command(command),
        Err(message) => {
            eprintln!("{message}\n\n{}", cli_usage());
            Err(2)
        }
    };

    match result {
        Ok(()) => Err(0),
        Err(code) => Err(code),
    }
}

fn run_cli_command(command: CliCommand) -> Result<(), i32> {
    match command {
        CliCommand::Screenshot { target } => match capture::take_screenshot(target) {
            Ok(path) => {
                println!("截图已保存: {}", path.display());
                Ok(())
            }
            Err(err) => {
                eprintln!("截图失败: {err}");
                Err(1)
            }
        },
        CliCommand::RecordStart { target, audio } => {
            match capture::start_recording_detached(target, audio) {
                Ok(path) => {
                    println!("录屏已开始，输出文件: {}", path.display());
                    Ok(())
                }
                Err(err) => {
                    eprintln!("开始录屏失败: {err}");
                    Err(1)
                }
            }
        }
        CliCommand::RecordStop => match capture::stop_recording_detached() {
            Ok(path) => {
                println!("录屏已停止，文件保存为: {}", path.display());
                Ok(())
            }
            Err(err) => {
                eprintln!("停止录屏失败: {err}");
                Err(1)
            }
        },
        CliCommand::Help => {
            println!("{}", cli_usage());
            Ok(())
        }
    }
}

fn parse_cli_command(args: &[String]) -> Result<CliCommand, String> {
    if args[0] == "help" || args[0] == "--help" || args[0] == "-h" {
        return Ok(CliCommand::Help);
    }

    if args[0] == "screenshot" {
        if args.len() != 2 {
            return Err("screenshot 命令格式错误".to_string());
        }

        let target = parse_target(&args[1])?;
        return Ok(CliCommand::Screenshot { target });
    }

    if args[0] == "record" {
        if args.len() >= 2 && args[1] == "start" {
            if args.len() < 3 || args.len() > 4 {
                return Err("record start 命令格式错误".to_string());
            }

            let target = parse_target(&args[2])?;
            let audio = if args.len() == 4 {
                if args[3] == "--audio" {
                    true
                } else {
                    return Err("record start 仅支持 --audio 参数".to_string());
                }
            } else {
                false
            };

            return Ok(CliCommand::RecordStart { target, audio });
        }

        if args.len() == 2 && args[1] == "stop" {
            return Ok(CliCommand::RecordStop);
        }

        return Err("record 命令格式错误".to_string());
    }

    Err("未知命令".to_string())
}

fn parse_target(input: &str) -> Result<CaptureTarget, String> {
    match input {
        "region" => Ok(CaptureTarget::Region),
        "fullscreen" => Ok(CaptureTarget::Fullscreen),
        _ => Err(format!("不支持的目标类型: {input}")),
    }
}

fn cli_usage() -> &'static str {
    "NCaptura CLI

用法:
  ncaptura                      启动图形界面
  ncaptura screenshot region
  ncaptura screenshot fullscreen
  ncaptura record start region [--audio]
  ncaptura record start fullscreen [--audio]
  ncaptura record stop
  ncaptura help

niri 快捷键示例:
  Mod+Shift+S    { spawn \"ncaptura\" \"screenshot\" \"region\"; }
  Mod+Shift+F    { spawn \"ncaptura\" \"screenshot\" \"fullscreen\"; }
  Mod+Shift+R    { spawn \"ncaptura\" \"record\" \"start\" \"region\"; }
  Mod+Shift+A    { spawn \"ncaptura\" \"record\" \"start\" \"region\" \"--audio\"; }
  Mod+Shift+E    { spawn \"ncaptura\" \"record\" \"stop\"; }"
}

fn build_ui(app: &Application) {
    let recording_state: Rc<RefCell<RecordingUiState>> =
        Rc::new(RefCell::new(RecordingUiState::default()));

    let window = ApplicationWindow::builder()
        .application(app)
        .title("NCaptura")
        .default_width(WINDOW_WIDTH)
        .default_height(WINDOW_HEIGHT)
        .resizable(false)
        .build();

    configure_window_placement(&window);

    let header_bar = HeaderBar::new();
    window.set_titlebar(Some(&header_bar));

    let content_box = GtkBox::new(Orientation::Vertical, 24);
    content_box.set_margin_top(24);
    content_box.set_margin_bottom(24);
    content_box.set_margin_start(24);
    content_box.set_margin_end(24);
    content_box.set_valign(Align::Center);

    let screenshot_box = GtkBox::new(Orientation::Vertical, 12);
    let screenshot_label = Label::new(Some("截图"));
    screenshot_label.add_css_class("title-4");
    screenshot_label.set_opacity(0.8);

    let screenshot_actions = GtkBox::new(Orientation::Horizontal, 16);
    screenshot_actions.set_halign(Align::Center);

    let screenshot_region_btn = build_icon_button("crop-symbolic", "区域截图");
    let screenshot_full_btn = build_icon_button("view-fullscreen-symbolic", "全屏截图");

    screenshot_actions.append(&screenshot_region_btn);
    screenshot_actions.append(&screenshot_full_btn);

    screenshot_box.append(&screenshot_label);
    screenshot_box.append(&screenshot_actions);

    let recording_box = GtkBox::new(Orientation::Vertical, 12);
    let recording_label = Label::new(Some("录屏"));
    recording_label.add_css_class("title-4");
    recording_label.set_opacity(0.8);

    let recording_actions = GtkBox::new(Orientation::Horizontal, 16);
    recording_actions.set_halign(Align::Center);

    let recording_region_btn = build_icon_button("media-record-symbolic", "区域录屏");
    recording_region_btn.add_css_class("suggested-action");

    let recording_full_btn = build_icon_button("video-x-generic-symbolic", "全屏录屏");
    recording_full_btn.add_css_class("suggested-action");

    recording_actions.append(&recording_region_btn);
    recording_actions.append(&recording_full_btn);

    let audio_toggle = ToggleButton::builder()
        .icon_name("audio-input-microphone-symbolic")
        .tooltip_text("录制系统音频")
        .halign(Align::Center)
        .build();
    audio_toggle.add_css_class("circular");

    let recording_controls = GtkBox::new(Orientation::Vertical, 12);
    recording_controls.append(&recording_actions);
    recording_controls.append(&audio_toggle);

    recording_box.append(&recording_label);
    recording_box.append(&recording_controls);

    let stop_recording_btn = Button::builder()
        .label("停止录屏")
        .icon_name("media-playback-stop-symbolic")
        .sensitive(false)
        .halign(Align::Center)
        .build();
    stop_recording_btn.add_css_class("destructive-action");
    stop_recording_btn.add_css_class("pill");

    let status_row = GtkBox::new(Orientation::Horizontal, 8);
    status_row.set_halign(Align::Center);

    let status_spinner = Spinner::new();
    status_spinner.set_visible(false);

    let status_label = Label::new(Some("就绪"));
    status_label.add_css_class("dim-label");
    status_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    status_label.set_width_chars(20);
    status_label.set_xalign(0.5);

    status_row.append(&status_spinner);
    status_row.append(&status_label);

    content_box.append(&screenshot_box);
    content_box.append(&recording_box);
    content_box.append(&stop_recording_btn);
    content_box.append(&status_row);

    window.set_child(Some(&content_box));

    {
        let status_label = status_label.clone();
        let status_spinner = status_spinner.clone();
        screenshot_region_btn.connect_clicked(move |_| {
            status_spinner.stop();
            status_spinner.set_visible(false);
            status_label.remove_css_class("dim-label");

            match capture::take_screenshot(CaptureTarget::Region) {
                Ok(path) => status_label.set_text(&format!(
                    "已保存: {}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                )),
                Err(_err) => status_label.set_text("截图失败"),
            }
        });
    }

    {
        let status_label = status_label.clone();
        let status_spinner = status_spinner.clone();
        screenshot_full_btn.connect_clicked(move |_| {
            status_spinner.stop();
            status_spinner.set_visible(false);
            status_label.remove_css_class("dim-label");

            match capture::take_screenshot(CaptureTarget::Fullscreen) {
                Ok(path) => status_label.set_text(&format!(
                    "已保存: {}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                )),
                Err(_err) => status_label.set_text("截图失败"),
            }
        });
    }

    {
        let recording_state = recording_state.clone();
        let audio_toggle = audio_toggle.clone();
        let status_label = status_label.clone();
        let status_spinner = status_spinner.clone();
        let rec_region_btn = recording_region_btn.clone();
        let rec_full_btn = recording_full_btn.clone();
        let stop_btn = stop_recording_btn.clone();

        recording_region_btn.connect_clicked(move |_| {
            start_recording(
                CaptureTarget::Region,
                &recording_state,
                &audio_toggle,
                &status_label,
                &status_spinner,
                &rec_region_btn,
                &rec_full_btn,
                &stop_btn,
            );
        });
    }

    {
        let recording_state = recording_state.clone();
        let audio_toggle = audio_toggle.clone();
        let status_label = status_label.clone();
        let status_spinner = status_spinner.clone();
        let rec_region_btn = recording_region_btn.clone();
        let rec_full_btn = recording_full_btn.clone();
        let stop_btn = stop_recording_btn.clone();

        recording_full_btn.connect_clicked(move |_| {
            start_recording(
                CaptureTarget::Fullscreen,
                &recording_state,
                &audio_toggle,
                &status_label,
                &status_spinner,
                &rec_region_btn,
                &rec_full_btn,
                &stop_btn,
            );
        });
    }

    {
        let recording_state = recording_state.clone();
        let audio_toggle = audio_toggle.clone();
        let status_label = status_label.clone();
        let status_spinner = status_spinner.clone();
        let rec_region_btn = recording_region_btn.clone();
        let rec_full_btn = recording_full_btn.clone();
        let stop_btn = stop_recording_btn.clone();

        stop_recording_btn.connect_clicked(move |_| {
            stop_recording(
                &recording_state,
                &audio_toggle,
                &status_label,
                &status_spinner,
                &rec_region_btn,
                &rec_full_btn,
                &stop_btn,
            );
        });
    }

    {
        let recording_state = recording_state.clone();
        window.connect_close_request(move |_| {
            clear_recording_ticker(&recording_state);
            if let Some(session) = recording_state.borrow_mut().session.take() {
                let _ = capture::stop_recording(session);
            }

            gtk::glib::Propagation::Proceed
        });
    }

    window.present();
}

fn build_icon_button(icon_name: &str, tooltip: &str) -> Button {
    let button = Button::builder()
        .icon_name(icon_name)
        .tooltip_text(tooltip)
        .build();
    button.add_css_class("circular");
    button.set_width_request(48);
    button.set_height_request(48);
    button
}

fn configure_window_placement(window: &ApplicationWindow) {
    if !gtk4_layer_shell::is_supported() {
        return;
    }

    window.init_layer_shell();
    window.set_layer(Layer::Top);
    window.set_anchor(Edge::Top, true);
    window.set_anchor(Edge::Right, true);
    window.set_margin(Edge::Top, WINDOW_MARGIN);
    window.set_margin(Edge::Right, WINDOW_MARGIN);
    window.set_keyboard_mode(KeyboardMode::OnDemand);
    window.set_namespace(Some("ncaptura"));

    if let Some(display) = gdk::Display::default()
        && let Some(monitor) = focused_monitor_from_niri(&display)
    {
        window.set_monitor(Some(&monitor));
    }
}

fn focused_monitor_from_niri(display: &gdk::Display) -> Option<gdk::Monitor> {
    let focused_output = capture::focused_output_name().ok()?;
    let monitors = display.monitors();

    for index in 0..monitors.n_items() {
        let Some(item) = monitors.item(index) else {
            continue;
        };

        let Ok(monitor) = item.downcast::<gdk::Monitor>() else {
            continue;
        };

        if monitor.connector().as_deref() == Some(focused_output.as_str()) {
            return Some(monitor);
        }
    }

    None
}

fn start_recording(
    target: CaptureTarget,
    recording_state: &Rc<RefCell<RecordingUiState>>,
    audio_toggle: &ToggleButton,
    status_label: &Label,
    status_spinner: &Spinner,
    rec_region_btn: &Button,
    rec_full_btn: &Button,
    stop_btn: &Button,
) {
    if recording_state.borrow().session.is_some() {
        status_label.set_text("已有录屏在进行中");
        return;
    }

    let with_audio = audio_toggle.is_active();
    match capture::start_recording(target, with_audio) {
        Ok(session) => {
            {
                let mut state = recording_state.borrow_mut();
                state.session = Some(session);
                state.started_at = Some(Instant::now());
                state.target = Some(target);
                state.with_audio = with_audio;
            }

            set_recording_controls(true, audio_toggle, rec_region_btn, rec_full_btn, stop_btn);

            status_label.remove_css_class("dim-label");
            status_label.set_text(&format_recording_status(0));
            status_spinner.set_visible(true);
            status_spinner.start();

            start_recording_ticker(recording_state, status_label, status_spinner);
        }
        Err(_err) => {
            status_spinner.stop();
            status_spinner.set_visible(false);
            status_label.remove_css_class("dim-label");
            status_label.set_text("开始录屏失败");
        }
    }
}

fn stop_recording(
    recording_state: &Rc<RefCell<RecordingUiState>>,
    audio_toggle: &ToggleButton,
    status_label: &Label,
    status_spinner: &Spinner,
    rec_region_btn: &Button,
    rec_full_btn: &Button,
    stop_btn: &Button,
) {
    let session = recording_state.borrow_mut().session.take();
    let Some(session) = session else {
        status_label.set_text("当前没有正在进行的录屏");
        return;
    };

    clear_recording_ticker(recording_state);

    {
        let mut state = recording_state.borrow_mut();
        state.started_at = None;
        state.target = None;
        state.with_audio = false;
    }

    match capture::stop_recording(session) {
        Ok(_path) => {
            status_spinner.stop();
            status_spinner.set_visible(false);
            status_label.remove_css_class("dim-label");
            status_label.set_text("录屏已保存");
        }
        Err(_err) => {
            status_spinner.stop();
            status_spinner.set_visible(false);
            status_label.remove_css_class("dim-label");
            status_label.set_text("停止录屏失败");
        }
    }

    set_recording_controls(false, audio_toggle, rec_region_btn, rec_full_btn, stop_btn);
}

fn set_recording_controls(
    is_recording: bool,
    audio_toggle: &ToggleButton,
    rec_region_btn: &Button,
    rec_full_btn: &Button,
    stop_btn: &Button,
) {
    rec_region_btn.set_sensitive(!is_recording);
    rec_full_btn.set_sensitive(!is_recording);
    audio_toggle.set_sensitive(!is_recording);
    stop_btn.set_sensitive(is_recording);
}

fn start_recording_ticker(
    recording_state: &Rc<RefCell<RecordingUiState>>,
    status_label: &Label,
    status_spinner: &Spinner,
) {
    clear_recording_ticker(recording_state);

    let recording_state = recording_state.clone();
    let ticker_state = recording_state.clone();
    let status_label = status_label.clone();
    let status_spinner = status_spinner.clone();

    let source_id = gtk::glib::timeout_add_local(Duration::from_secs(1), move || {
        let (recording_active, started_at) = {
            let state = ticker_state.borrow();
            (state.session.is_some(), state.started_at)
        };

        if !recording_active {
            status_spinner.stop();
            status_spinner.set_visible(false);
            return gtk::glib::ControlFlow::Break;
        }

        let Some(started_at) = started_at else {
            return gtk::glib::ControlFlow::Continue;
        };

        let elapsed_seconds = started_at.elapsed().as_secs();
        status_spinner.set_visible(true);
        status_spinner.start();
        status_label.remove_css_class("dim-label");
        status_label.set_text(&format_recording_status(elapsed_seconds));

        gtk::glib::ControlFlow::Continue
    });

    recording_state.borrow_mut().ticker = Some(source_id);
}

fn clear_recording_ticker(recording_state: &Rc<RefCell<RecordingUiState>>) {
    if let Some(source_id) = recording_state.borrow_mut().ticker.take() {
        source_id.remove();
    }
}

fn format_recording_status(elapsed_seconds: u64) -> String {
    let hours = elapsed_seconds / 3600;
    let minutes = (elapsed_seconds % 3600) / 60;
    let seconds = elapsed_seconds % 60;

    format!("{}:{:02}:{:02}", hours, minutes, seconds,)
}

enum CliCommand {
    Screenshot { target: CaptureTarget },
    RecordStart { target: CaptureTarget, audio: bool },
    RecordStop,
    Help,
}
