use std::path::PathBuf;
use std::time::Duration;

use adw::prelude::*;
use gtk::gdk_pixbuf::Pixbuf;

use crate::capture::{
    CaptureTarget, is_window_protocol_unsupported_error, list_windows, take_screenshot,
    take_window_screenshot, take_window_screenshot_via_niri,
};
use crate::ui::{
    CaptureMode, InteractiveDialogResult, build_interactive_dialog, build_save_dialog,
    show_window_picker,
};

pub fn run() {
    let app = adw::Application::builder()
        .application_id("io.ncaptura.app")
        .build();

    app.connect_activate(build_ui);
    app.run();
}

fn build_ui(app: &adw::Application) {
    let app_clone = app.clone();
    let _window = build_interactive_dialog(app, move |result| {
        let guard = app_clone.hold();
        perform_capture(&app_clone, &result, guard);
    });
}

fn perform_capture(
    app: &adw::Application,
    result: &InteractiveDialogResult,
    guard: gtk::gio::ApplicationHoldGuard,
) {
    let _ = result.show_pointer;

    match result.mode {
        CaptureMode::Screen => {
            schedule_target_capture(app, CaptureTarget::Fullscreen, result.delay_seconds, guard);
        }
        CaptureMode::Selection => {
            schedule_target_capture(app, CaptureTarget::Region, result.delay_seconds, guard);
        }
        CaptureMode::Window => {
            show_window_picker_for_capture(app, result.delay_seconds, guard);
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

fn show_window_picker_for_capture(
    app: &adw::Application,
    delay_seconds: u32,
    guard: gtk::gio::ApplicationHoldGuard,
) {
    let mut windows = match list_windows() {
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

    let picker_app = app.clone();
    let capture_app = app.clone();
    show_window_picker(&picker_app, windows, guard, move |window_id, guard| {
        if delay_seconds > 0 {
            let app = capture_app.clone();
            gtk::glib::timeout_add_local_once(
                Duration::from_secs(delay_seconds as u64),
                move || {
                    take_window_and_show(&app, window_id, guard);
                },
            );
        } else {
            take_window_and_show(&capture_app, window_id, guard);
        }
    });
}

fn take_and_show(
    app: &adw::Application,
    target: CaptureTarget,
    _guard: gtk::gio::ApplicationHoldGuard,
) {
    let path = match take_screenshot(target) {
        Ok(path) => path,
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
    let path = match take_window_screenshot(window_id, false) {
        Ok(path) => path,
        Err(err) => {
            if is_window_protocol_unsupported_error(&err) {
                if let Err(niri_err) = take_window_screenshot_via_niri(window_id) {
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
        Ok(pixbuf) => pixbuf,
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

    build_save_dialog(app, &pixbuf, &folder, &filename);
}
