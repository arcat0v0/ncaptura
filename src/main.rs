mod capture;

use std::cell::RefCell;
use std::env;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, CheckButton, HeaderBar, Label,
    Orientation,
};

use crate::capture::{CaptureTarget, RecordingSession};

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

enum CliCommand {
    Screenshot { target: CaptureTarget },
    RecordStart { target: CaptureTarget, audio: bool },
    RecordStop,
    Help,
}

fn build_ui(app: &Application) {
    let recording_state: Rc<RefCell<Option<RecordingSession>>> = Rc::new(RefCell::new(None));

    let window = ApplicationWindow::builder()
        .application(app)
        .title("NCaptura")
        .default_width(360)
        .default_height(420)
        .build();

    let header_bar = HeaderBar::new();
    window.set_titlebar(Some(&header_bar));

    let content_box = GtkBox::new(Orientation::Vertical, 24);
    content_box.set_margin_top(24);
    content_box.set_margin_bottom(24);
    content_box.set_margin_start(24);
    content_box.set_margin_end(24);
    content_box.set_valign(Align::Start);

    let screenshot_box = GtkBox::new(Orientation::Vertical, 12);

    let screenshot_label = Label::builder()
        .label("<b>截图</b>")
        .use_markup(true)
        .halign(Align::Start)
        .build();
    screenshot_box.append(&screenshot_label);

    let screenshot_buttons_box = GtkBox::new(Orientation::Horizontal, 12);
    screenshot_buttons_box.add_css_class("linked");
    screenshot_buttons_box.set_halign(Align::Fill);

    let screenshot_region_btn = Button::builder().label("区域截图").hexpand(true).build();
    let screenshot_full_btn = Button::builder().label("全屏截图").hexpand(true).build();

    screenshot_buttons_box.append(&screenshot_region_btn);
    screenshot_buttons_box.append(&screenshot_full_btn);
    screenshot_box.append(&screenshot_buttons_box);

    content_box.append(&screenshot_box);

    let recording_box = GtkBox::new(Orientation::Vertical, 12);

    let recording_label = Label::builder()
        .label("<b>录屏</b>")
        .use_markup(true)
        .halign(Align::Start)
        .build();
    recording_box.append(&recording_label);

    let audio_checkbox = CheckButton::with_label("包含音频（系统混音）");
    recording_box.append(&audio_checkbox);

    let recording_buttons_box = GtkBox::new(Orientation::Horizontal, 12);
    recording_buttons_box.add_css_class("linked");

    let recording_region_btn = Button::builder().label("区域录屏").hexpand(true).build();
    recording_region_btn.add_css_class("suggested-action");

    let recording_full_btn = Button::builder().label("全屏录屏").hexpand(true).build();
    recording_full_btn.add_css_class("suggested-action");

    recording_buttons_box.append(&recording_region_btn);
    recording_buttons_box.append(&recording_full_btn);
    recording_box.append(&recording_buttons_box);

    let stop_recording_btn = Button::builder().label("停止录屏").sensitive(false).build();
    stop_recording_btn.add_css_class("destructive-action");
    recording_box.append(&stop_recording_btn);

    content_box.append(&recording_box);

    let status_label = Label::builder()
        .label("就绪")
        .halign(Align::Center)
        .css_classes(vec!["dim-label"])
        .build();

    content_box.append(&status_label);

    window.set_child(Some(&content_box));

    {
        let status_label = status_label.clone();
        screenshot_region_btn.connect_clicked(move |_| {
            match capture::take_screenshot(CaptureTarget::Region) {
                Ok(path) => status_label.set_text(&format!("区域截图成功: {}", path.display())),
                Err(err) => status_label.set_text(&format!("区域截图失败: {err}")),
            }
        });
    }

    {
        let status_label = status_label.clone();
        screenshot_full_btn.connect_clicked(move |_| {
            match capture::take_screenshot(CaptureTarget::Fullscreen) {
                Ok(path) => status_label.set_text(&format!("全屏截图成功: {}", path.display())),
                Err(err) => status_label.set_text(&format!("全屏截图失败: {err}")),
            }
        });
    }

    {
        let recording_state = recording_state.clone();
        let audio_checkbox = audio_checkbox.clone();
        let status_label = status_label.clone();
        let recording_region_btn_ref = recording_region_btn.clone();
        let recording_full_btn_ref = recording_full_btn.clone();
        let stop_recording_btn_ref = stop_recording_btn.clone();

        recording_region_btn.connect_clicked(move |_| {
            start_recording(
                CaptureTarget::Region,
                &recording_state,
                &audio_checkbox,
                &status_label,
                &recording_region_btn_ref,
                &recording_full_btn_ref,
                &stop_recording_btn_ref,
            );
        });
    }

    {
        let recording_state = recording_state.clone();
        let audio_checkbox = audio_checkbox.clone();
        let status_label = status_label.clone();
        let recording_region_btn_ref = recording_region_btn.clone();
        let recording_full_btn_ref = recording_full_btn.clone();
        let stop_recording_btn_ref = stop_recording_btn.clone();

        recording_full_btn.connect_clicked(move |_| {
            start_recording(
                CaptureTarget::Fullscreen,
                &recording_state,
                &audio_checkbox,
                &status_label,
                &recording_region_btn_ref,
                &recording_full_btn_ref,
                &stop_recording_btn_ref,
            );
        });
    }

    {
        let recording_state = recording_state.clone();
        let status_label = status_label.clone();
        let recording_region_btn_ref = recording_region_btn.clone();
        let recording_full_btn_ref = recording_full_btn.clone();
        let stop_recording_btn_ref = stop_recording_btn.clone();

        stop_recording_btn.connect_clicked(move |_| {
            stop_recording(
                &recording_state,
                &status_label,
                &recording_region_btn_ref,
                &recording_full_btn_ref,
                &stop_recording_btn_ref,
            );
        });
    }

    {
        let recording_state = recording_state.clone();
        window.connect_close_request(move |_| {
            if let Some(session) = recording_state.borrow_mut().take() {
                let _ = capture::stop_recording(session);
            }
            gtk::glib::Propagation::Proceed
        });
    }

    window.present();
}

fn start_recording(
    target: CaptureTarget,
    recording_state: &Rc<RefCell<Option<RecordingSession>>>,
    audio_checkbox: &CheckButton,
    status_label: &Label,
    recording_region_btn: &Button,
    recording_full_btn: &Button,
    stop_recording_btn: &Button,
) {
    if recording_state.borrow().is_some() {
        status_label.set_text("已有录屏正在进行，请先停止当前录屏");
        return;
    }

    let with_audio = audio_checkbox.is_active();

    match capture::start_recording(target, with_audio) {
        Ok(session) => {
            *recording_state.borrow_mut() = Some(session);
            recording_region_btn.set_sensitive(false);
            recording_full_btn.set_sensitive(false);
            stop_recording_btn.set_sensitive(true);
            status_label.set_text(&format!(
                "已开始{}录屏{}",
                target_label(target),
                if with_audio { "（含音频）" } else { "" }
            ));
        }
        Err(err) => {
            status_label.set_text(&format!("开始录屏失败: {err}"));
        }
    }
}

fn stop_recording(
    recording_state: &Rc<RefCell<Option<RecordingSession>>>,
    status_label: &Label,
    recording_region_btn: &Button,
    recording_full_btn: &Button,
    stop_recording_btn: &Button,
) {
    let session = recording_state.borrow_mut().take();

    let Some(session) = session else {
        status_label.set_text("当前没有正在进行的录屏");
        return;
    };

    match capture::stop_recording(session) {
        Ok(path) => {
            status_label.set_text(&format!("录屏已保存: {}", path.display()));
        }
        Err(err) => {
            status_label.set_text(&format!("停止录屏失败: {err}"));
        }
    }

    recording_region_btn.set_sensitive(true);
    recording_full_btn.set_sensitive(true);
    stop_recording_btn.set_sensitive(false);
}

fn target_label(target: CaptureTarget) -> &'static str {
    match target {
        CaptureTarget::Region => "区域",
        CaptureTarget::Fullscreen => "全屏",
    }
}
