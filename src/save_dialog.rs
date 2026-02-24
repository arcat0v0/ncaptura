use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use adw::prelude::*;
use gtk::gdk;
use gtk::gdk::prelude::GdkCairoContextExt;
use gtk::gdk_pixbuf::Pixbuf;

pub struct SaveDialogResult {
    pub folder: PathBuf,
    pub filename: String,
}

pub fn build_save_dialog(
    app: &adw::Application,
    screenshot: &Pixbuf,
    initial_folder: &PathBuf,
    initial_filename: &str,
) -> adw::ApplicationWindow {
    let selected_folder = Rc::new(RefCell::new(initial_folder.clone()));
    let result: Rc<RefCell<Option<SaveDialogResult>>> = Rc::new(RefCell::new(None));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Save Screenshot")
        .default_width(640)
        .default_height(520)
        .resizable(true)
        .build();

    let header = adw::HeaderBar::new();

    let cancel_button = gtk::Button::with_label("Cancel");
    {
        let window = window.clone();
        cancel_button.connect_clicked(move |_| {
            window.close();
        });
    }
    header.pack_start(&cancel_button);

    let copy_button = gtk::Button::with_label("Copy to Clipboard");
    {
        let screenshot = screenshot.clone();
        copy_button.connect_clicked(move |_| {
            if let Some(display) = gdk::Display::default() {
                let clipboard = display.clipboard();
                let texture = gdk::Texture::for_pixbuf(&screenshot);
                clipboard.set_texture(&texture);
            }
        });
    }
    header.pack_end(&copy_button);

    let save_button = gtk::Button::with_label("Save");
    save_button.add_css_class("suggested-action");
    window.set_default_widget(Some(&save_button));
    header.pack_end(&save_button);

    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.append(&header);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 24);
    content.set_halign(gtk::Align::Fill);
    content.set_valign(gtk::Align::Fill);
    content.set_hexpand(true);
    content.set_vexpand(true);
    content.set_margin_top(24);
    content.set_margin_bottom(24);
    content.set_margin_start(24);
    content.set_margin_end(24);

    let preview_area = gtk::DrawingArea::new();
    preview_area.set_width_request(256);
    preview_area.set_height_request(256);
    preview_area.set_hexpand(true);
    preview_area.set_vexpand(true);
    {
        let screenshot = screenshot.clone();
        preview_area.set_draw_func(move |_, cr, width, height| {
            let source_width = screenshot.width() as f64;
            let source_height = screenshot.height() as f64;
            if source_width <= 0.0 || source_height <= 0.0 {
                return;
            }

            let target_width = width as f64;
            let target_height = height as f64;
            let scale = f64::min(target_width / source_width, target_height / source_height);
            let draw_width = source_width * scale;
            let draw_height = source_height * scale;
            let offset_x = (target_width - draw_width) / 2.0;
            let offset_y = (target_height - draw_height) / 2.0;

            cr.save().ok();
            cr.translate(offset_x, offset_y);
            cr.scale(scale, scale);
            cr.set_source_pixbuf(&screenshot, 0.0, 0.0);
            let _ = cr.paint();
            cr.restore().ok();
        });
    }
    content.append(&preview_area);

    let form_grid = gtk::Grid::new();
    form_grid.set_halign(gtk::Align::Center);
    form_grid.set_row_spacing(6);
    form_grid.set_column_spacing(12);

    let name_label = gtk::Label::new(Some("Name:"));
    name_label.set_halign(gtk::Align::End);

    let name_entry = gtk::Entry::new();
    name_entry.set_width_chars(35);
    name_entry.set_activates_default(true);
    name_entry.set_text(initial_filename);

    let selected_char_count = selected_filename_chars(initial_filename);
    name_entry.select_region(0, selected_char_count);

    let folder_label = gtk::Label::new(Some("Folder:"));
    folder_label.set_halign(gtk::Align::End);

    let folder_button = gtk::Button::with_label(&initial_folder.to_string_lossy());
    folder_button.set_halign(gtk::Align::Fill);

    {
        let window = window.clone();
        let folder_button_handle = folder_button.clone();
        let folder_button = folder_button.clone();
        let selected_folder = selected_folder.clone();
        folder_button_handle.connect_clicked(move |_| {
            let chooser = gtk::FileChooserNative::builder()
                .title("Select Folder")
                .action(gtk::FileChooserAction::SelectFolder)
                .transient_for(&window)
                .modal(true)
                .build();

            let folder_button = folder_button.clone();
            let selected_folder = selected_folder.clone();
            chooser.connect_response(move |chooser, response| {
                if response == gtk::ResponseType::Accept {
                    if let Some(file) = chooser.file() {
                        if let Some(path) = file.path() {
                            *selected_folder.borrow_mut() = path.clone();
                            folder_button.set_label(&path.to_string_lossy());
                        }
                    }
                }
            });
            chooser.show();
        });
    }

    {
        let window = window.clone();
        let name_entry = name_entry.clone();
        let selected_folder = selected_folder.clone();
        let result = result.clone();
        save_button.connect_clicked(move |_| {
            let filename = name_entry.text().to_string();
            let folder = selected_folder.borrow().clone();
            *result.borrow_mut() = Some(SaveDialogResult { folder, filename });
            window.close();
        });
    }

    form_grid.attach(&name_label, 0, 0, 1, 1);
    form_grid.attach(&name_entry, 1, 0, 1, 1);
    form_grid.attach(&folder_label, 0, 1, 1, 1);
    form_grid.attach(&folder_button, 1, 1, 1, 1);

    content.append(&form_grid);
    root.append(&content);
    window.set_content(Some(&root));

    let key_controller = gtk::EventControllerKey::new();
    {
        let window = window.clone();
        key_controller.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape {
                window.close();
                return gtk::glib::Propagation::Stop;
            }

            gtk::glib::Propagation::Proceed
        });
    }
    window.add_controller(key_controller);

    window.present();
    window
}

fn selected_filename_chars(filename: &str) -> i32 {
    let stem = Path::new(filename)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(filename);

    stem.chars().count() as i32
}
