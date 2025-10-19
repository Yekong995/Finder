use std::error::Error;
use std::rc::Rc;
use std::thread;

use clipboard_rs::{Clipboard, ClipboardContext};
use finder::*;
use rfd::{FileDialog, MessageDialog};
use slint::{Model, Weak};
use slint::{SharedString, StandardListViewItem, VecModel};
use walkdir::WalkDir;

slint::include_modules!();
pub fn main() -> Result<(), Box<dyn Error>> {
    let app = MyWindow::new()?;

    // Handle file selection
    let pick_handle: Weak<MyWindow> = app.as_weak();
    app.on_select_file(move || {
        let ui = pick_handle.unwrap();

        if let Some(path) = FileDialog::new().pick_folder() {
            ui.set_file_path(path.to_string_lossy().into_owned().into());
        }
    });

    // Implement fuzzy search and return top 10 results
    let search_handle: Weak<MyWindow> = app.as_weak();
    app.on_search_data(move || {
        let ui = search_handle.upgrade().unwrap();
        let folder_path = ui.get_file_path().to_string();
        let input = ui.get_search_txt().to_string();

        if folder_path == "" {
            MessageDialog::new()
                .set_title("Warning")
                .set_level(rfd::MessageLevel::Warning)
                .set_buttons(rfd::MessageButtons::Ok)
                .set_description("Please choose a folder for searching")
                .show();
            return;
        }

        let total = WalkDir::new(folder_path.clone())
            .into_iter()
            .filter_map(Result::ok)
            .count();

        if total == 0 {
            ui.set_prg(1.0);
        } else {
            let ui_weak = search_handle.clone();
            thread::spawn(move || {
                let mut count = 0;
                let mut list: Vec<String> = Vec::new();

                for entry in WalkDir::new(folder_path.clone()).into_iter() {
                    if entry.is_err() {
                        continue;
                    }
                    let entry = entry.unwrap();
                    let path = entry.path();
                    let path = path.to_str().unwrap();
                    count += 1;

                    let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        ui.set_prg(count as f32 / total as f32);
                    });

                    list.push(path.to_string());
                }

                let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    ui.set_prg(1.0);
                    let matched: Vec<String> = fuzzy(list, input).unwrap();

                    let matched: Vec<StandardListViewItem> = matched
                        .iter()
                        .map(|x| {
                            let item = StandardListViewItem::from(SharedString::from(x));
                            item
                        })
                        .collect();

                    let model: Rc<VecModel<StandardListViewItem>> =
                        Rc::new(VecModel::from(matched));
                    ui.set_list_data(model.clone().into());
                });
            });
        }
    });

    // Implement copy function
    let copy_handle: Weak<MyWindow> = app.as_weak();
    app.on_select_item(move |index| {
        let ui = copy_handle.unwrap();
        let row = index as i64;

        if row < 0 {
            MessageDialog::new()
                .set_title("Warning")
                .set_level(rfd::MessageLevel::Warning)
                .set_buttons(rfd::MessageButtons::Ok)
                .set_description("Please select a valid item")
                .show();
            return;
        }

        let list_path = ui.get_list_data();
        let mut txt: String = "".to_string();
        if let Some(vec_model) = list_path
            .as_any()
            .downcast_ref::<VecModel<StandardListViewItem>>()
        {
            let tmp = vec_model.row_data(row as usize);
            txt = tmp.unwrap().text.to_string();
        }

        let ctx = ClipboardContext::new().unwrap();
        ctx.set_text(txt).unwrap();
    });

    let _ = app.run();

    Ok(())
}
