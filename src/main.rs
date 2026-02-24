mod capture;
mod interactive_dialog;
mod save_dialog;

use std::env;
use std::path::PathBuf;
use std::rc::Rc;
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
    match result.mode {
        CaptureMode::Screen => {
            schedule_target_capture(app, CaptureTarget::Fullscreen, result.delay_seconds, guard);
        }
        CaptureMode::Selection => {
            schedule_target_capture(app, CaptureTarget::Region, result.delay_seconds, guard);
        }
        CaptureMode::Window => {
            show_window_picker(app, result.delay_seconds, guard);
        }
    }
}

fn schedule_target_capture(
    app: &adw::Application,
    target: CaptureTarget,
    delay_seconds: u32,
    guard: gtk::gio::ApplicationHoldGuard,
) {
    if delay_seconds > 0 {
        let app = app.clone();
        gtk::glib::timeout_add_local_once(Duration::from_secs(delay_seconds as u64), move || {
            take_and_show(&app, target, guard);
        });
    } else {
        take_and_show(app, target, guard);
    }
}

fn show_window_picker(
    app: &adw::Application,
    delay_seconds: u32,
    guard: gtk::gio::ApplicationHoldGuard,
) {
    let mut windows = match capture::list_windows() {
        Ok(items) => items,
        Err(err) => {
            eprintln!("读取窗口列表失败: {err}");
            return;
        }
    };

    windows.retain(|w| w.app_id != "io.ncaptura.app");
    if windows.is_empty() {
        eprintln!("没有可供选择的窗口");
        return;
    }

    let picker = adw::ApplicationWindow::builder()
        .application(app)
        .title("Select Window")
        .default_width(560)
        .default_height(440)
        .resizable(false)
        .build();

    let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
    root.set_margin_top(16);
    root.set_margin_bottom(16);
    root.set_margin_start(16);
    root.set_margin_end(16);

    let hint = gtk::Label::new(Some("选择要截图的窗口"));
    hint.set_halign(gtk::Align::Start);
    root.append(&hint);

    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::Single);
    list.add_css_class("boxed-list");
    list.set_vexpand(true);

    for window in &windows {
        let row = gtk::ListBoxRow::new();
        let row_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
        row_box.set_margin_top(8);
        row_box.set_margin_bottom(8);
        row_box.set_margin_start(8);
        row_box.set_margin_end(8);

        let title = gtk::Label::new(Some(&window.title));
        title.set_halign(gtk::Align::Start);
        title.set_wrap(true);

        let subtitle = gtk::Label::new(Some(&format!(
            "{}  |  workspace {}  |  id {}",
            window.app_id, window.workspace_id, window.id
        )));
        subtitle.set_halign(gtk::Align::Start);
        subtitle.add_css_class("dim-label");

        row_box.append(&title);
        row_box.append(&subtitle);
        row.set_child(Some(&row_box));
        list.append(&row);
    }

    root.append(&list);

    let action_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    action_row.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label("Cancel");
    let capture_btn = gtk::Button::with_label("Take Screenshot");
    capture_btn.add_css_class("suggested-action");
    action_row.append(&cancel);
    action_row.append(&capture_btn);
    root.append(&action_row);

    picker.set_content(Some(&root));

    let windows = Rc::new(windows);
    let selected_index = Rc::new(std::cell::RefCell::new(Some(0usize)));
    list.select_row(list.row_at_index(0).as_ref());

    {
        let selected_index = selected_index.clone();
        list.connect_selected_rows_changed(move |listbox| {
            let row = listbox.selected_row();
            *selected_index.borrow_mut() = row.map(|r| r.index() as usize);
        });
    }

    let guard_cell = Rc::new(std::cell::RefCell::new(Some(guard)));

    {
        let picker = picker.clone();
        let guard_cell = guard_cell.clone();
        cancel.connect_clicked(move |_| {
            picker.destroy();
            let _ = guard_cell.borrow_mut().take();
        });
    }

    {
        let picker = picker.clone();
        let app = app.clone();
        let windows = windows.clone();
        let selected_index = selected_index.clone();
        let guard_cell = guard_cell.clone();
        capture_btn.connect_clicked(move |_| {
            let Some(idx) = *selected_index.borrow() else {
                return;
            };
            let Some(info) = windows.get(idx) else {
                return;
            };
            let Some(guard) = guard_cell.borrow_mut().take() else {
                return;
            };

            picker.destroy();
            if delay_seconds > 0 {
                let app = app.clone();
                let window_id = info.id;
                gtk::glib::timeout_add_local_once(
                    Duration::from_secs(delay_seconds as u64),
                    move || {
                        take_window_and_show(&app, window_id, guard);
                    },
                );
            } else {
                take_window_and_show(&app, info.id, guard);
            }
        });
    }

    picker.present();
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

    show_save_dialog_for_path(app, path);
}

fn take_window_and_show(
    app: &adw::Application,
    window_id: u64,
    _guard: gtk::gio::ApplicationHoldGuard,
) {
    let path = match capture::take_window_screenshot(window_id, false) {
        Ok(p) => p,
        Err(err) => {
            if capture::is_window_protocol_unsupported_error(&err) {
                if let Err(niri_err) = capture::take_window_screenshot_via_niri(window_id) {
                    eprintln!("窗口截图失败: {niri_err}");
                }
                return;
            }
            eprintln!("窗口截图失败: {err}");
            return;
        }
    };

    show_save_dialog_for_path(app, path);
}

fn show_save_dialog_for_path(app: &adw::Application, path: PathBuf) {
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
