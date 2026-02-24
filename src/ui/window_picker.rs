use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;

use crate::capture::WindowInfo;

pub fn show_window_picker(
    app: &adw::Application,
    windows: Vec<WindowInfo>,
    guard: gtk::gio::ApplicationHoldGuard,
    on_capture: impl Fn(u64, gtk::gio::ApplicationHoldGuard) + 'static,
) {
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
    let selected_index = Rc::new(RefCell::new(Some(0usize)));
    list.select_row(list.row_at_index(0).as_ref());

    {
        let selected_index = selected_index.clone();
        list.connect_selected_rows_changed(move |listbox| {
            let row = listbox.selected_row();
            *selected_index.borrow_mut() = row.map(|r| r.index() as usize);
        });
    }

    let guard_cell = Rc::new(RefCell::new(Some(guard)));

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
            on_capture(info.id, guard);
        });
    }

    picker.present();
}
