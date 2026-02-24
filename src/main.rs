mod capture;
mod interactive_dialog;
mod save_dialog;

use std::env;
use std::path::PathBuf;
use std::time::Duration;

use adw::prelude::*;
use gtk::gdk_pixbuf::Pixbuf;

use crate::capture::CaptureTarget;
use crate::interactive_dialog::CaptureMode;

fn main() {
    if let Err(code) = handle_cli_if_requested() {
        std::process::exit(code);
    }

    let app = adw::Application::builder()
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

fn build_ui(app: &adw::Application) {
    let app_clone = app.clone();
    let _window = interactive_dialog::build_interactive_dialog(app, move |result| {
        let _guard = app_clone.hold();
        perform_capture(&app_clone, &result, _guard);
    });
}

fn perform_capture(
    app: &adw::Application,
    result: &interactive_dialog::InteractiveDialogResult,
    guard: gtk::gio::ApplicationHoldGuard,
) {
    let target = match result.mode {
        CaptureMode::Screen => CaptureTarget::Fullscreen,
        CaptureMode::Window => CaptureTarget::Region,
        CaptureMode::Selection => CaptureTarget::Region,
    };

    if result.delay_seconds > 0 {
        let delay = result.delay_seconds;
        let app = app.clone();
        gtk::glib::timeout_add_local_once(Duration::from_secs(delay as u64), move || {
            take_and_show(&app, target, guard);
        });
    } else {
        take_and_show(app, target, guard);
    }
}

fn take_and_show(
    app: &adw::Application,
    target: CaptureTarget,
    _guard: gtk::gio::ApplicationHoldGuard,
) {
    let path = match capture::take_screenshot(target) {
        Ok(p) => p,
        Err(err) => {
            eprintln!("截图失败: {err}");
            return;
        }
    };

    let pixbuf = match Pixbuf::from_file(&path) {
        Ok(p) => p,
        Err(err) => {
            eprintln!("无法加载截图: {err}");
            return;
        }
    };

    let folder = path.parent().map(PathBuf::from).unwrap_or_default();
    let filename = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    save_dialog::build_save_dialog(app, &pixbuf, &folder, &filename);
}

enum CliCommand {
    Screenshot { target: CaptureTarget },
    RecordStart { target: CaptureTarget, audio: bool },
    RecordStop,
    Help,
}
