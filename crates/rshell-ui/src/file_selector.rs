use std::{cell::RefCell, rc::Rc};

use gtk::prelude::*;
use relm4::gtk;
use rshell_platform::{
    FileSelectionCallback, FileSelectionError, FileSelectionPurpose, FileSelectionRequest,
    FileSelectionService,
};

#[derive(Default)]
pub struct GtkFileSelectionService;

impl FileSelectionService for GtkFileSelectionService {
    fn select_file(&self, request: FileSelectionRequest, complete: FileSelectionCallback) {
        let chooser = gtk::FileChooserNative::builder()
            .action(gtk::FileChooserAction::Open)
            .title(request.title)
            .accept_label("Open")
            .cancel_label("Cancel")
            .build();
        let filter = gtk::FileFilter::new();
        match request.purpose {
            FileSelectionPurpose::LegacyRshellImport => {
                filter.set_name(Some("JSON files"));
                filter.add_mime_type("application/json");
                filter.add_pattern("*.json");
            }
            FileSelectionPurpose::OpenSshImport => {
                filter.set_name(Some("OpenSSH configuration"));
                filter.add_pattern("config");
                filter.add_pattern("*.conf");
                filter.add_pattern("*");
            }
        }
        chooser.add_filter(&filter);

        let complete = Rc::new(RefCell::new(Some(complete)));
        let keep_alive = chooser.clone();
        chooser.connect_response(move |dialog, response| {
            let result = if response == gtk::ResponseType::Accept {
                dialog
                    .file()
                    .and_then(|file| file.path())
                    .map(Some)
                    .ok_or(FileSelectionError::InvalidSelection)
            } else {
                Ok(None)
            };
            if let Some(complete) = complete.borrow_mut().take() {
                complete(result);
            }
            keep_alive.destroy();
        });
        chooser.show();
    }
}
