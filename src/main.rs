mod capture;

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Box as GtkBox, Button, CheckButton, Label, Orientation};

use crate::capture::{CaptureTarget, RecordingSession};

fn main() {
    let app = Application::builder()
        .application_id("io.ncaptura.app")
        .build();

    app.connect_activate(build_ui);
    app.run();
}

fn build_ui(app: &Application) {
    let recording_state: Rc<RefCell<Option<RecordingSession>>> = Rc::new(RefCell::new(None));

    let window = ApplicationWindow::builder()
        .application(app)
        .title("NCaptura")
        .default_width(480)
        .default_height(320)
        .build();

    let root = GtkBox::new(Orientation::Vertical, 12);
    root.set_margin_top(16);
    root.set_margin_bottom(16);
    root.set_margin_start(16);
    root.set_margin_end(16);

    let title = Label::new(Some("NCaptura - Basic Capture"));
    title.set_xalign(0.0);

    let audio_checkbox = CheckButton::with_label("录屏包含音频（优先系统混音）");
    audio_checkbox.set_active(false);

    let screenshot_region_btn = Button::with_label("区域截图");
    let screenshot_full_btn = Button::with_label("全屏截图");

    let recording_region_btn = Button::with_label("开始区域录屏");
    let recording_full_btn = Button::with_label("开始全屏录屏");
    let stop_recording_btn = Button::with_label("停止录屏");
    stop_recording_btn.set_sensitive(false);

    let status_label = Label::new(Some("就绪"));
    status_label.set_xalign(0.0);

    root.append(&title);
    root.append(&audio_checkbox);
    root.append(&screenshot_region_btn);
    root.append(&screenshot_full_btn);
    root.append(&recording_region_btn);
    root.append(&recording_full_btn);
    root.append(&stop_recording_btn);
    root.append(&status_label);

    {
        let status_label = status_label.clone();
        screenshot_region_btn.connect_clicked(move |_| {
            match capture::take_screenshot(CaptureTarget::Region) {
                Ok(path) => {
                    status_label.set_text(&format!("区域截图成功: {}", path.display()));
                }
                Err(err) => {
                    status_label.set_text(&format!("区域截图失败: {err}"));
                }
            }
        });
    }

    {
        let status_label = status_label.clone();
        screenshot_full_btn.connect_clicked(move |_| {
            match capture::take_screenshot(CaptureTarget::Fullscreen) {
                Ok(path) => {
                    status_label.set_text(&format!("全屏截图成功: {}", path.display()));
                }
                Err(err) => {
                    status_label.set_text(&format!("全屏截图失败: {err}"));
                }
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

    window.set_child(Some(&root));
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
