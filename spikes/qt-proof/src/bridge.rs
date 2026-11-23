//! The Rust side of the proof: a QObject that lists a folder with
//! steward-core's FolderModel (GIO, running on the GLib main loop that Qt itself
//! runs on Linux) and hands the result to QML.

use std::cell::RefCell;
use std::pin::Pin;

use steward_core::sort::{self, SortKey};
use steward_core::{FileEntry, FolderModel, LoadState};
use cxx_qt::Threading;
use cxx_qt_lib::QString;
use gio::prelude::*;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, names)]
        #[qproperty(QString, kinds)]
        #[qproperty(QString, sizes)]
        #[qproperty(QString, status)]
        #[qproperty(QString, folder)]
        #[qproperty(QString, capture)]
        type FolderLister = super::FolderListerRust;

        #[qinvokable]
        fn load(self: Pin<&mut FolderLister>, path: &QString);
    }

    impl cxx_qt::Threading for FolderLister {}
    impl cxx_qt::Initialize for FolderLister {}
}

#[derive(Default)]
pub struct FolderListerRust {
    names: QString,
    kinds: QString,
    sizes: QString,
    status: QString,
    folder: QString,
    capture: QString,
}

thread_local! {
    // Listings stay alive while the app runs.
    static MODELS: RefCell<Vec<FolderModel>> = const { RefCell::new(Vec::new()) };
}

impl cxx_qt::Initialize for qobject::FolderLister {
    fn initialize(mut self: Pin<&mut Self>) {
        let folder = std::env::var("STEWARD_PROOF_FOLDER").unwrap_or_default();
        let capture = std::env::var("STEWARD_CAPTURE_DIR").unwrap_or_default();
        self.as_mut().set_folder(QString::from(folder.as_str()));
        self.as_mut().set_capture(QString::from(capture.as_str()));
        self.set_status(QString::from("idle"));
    }
}

impl qobject::FolderLister {
    pub fn load(mut self: Pin<&mut Self>, path: &QString) {
        let model = FolderModel::new();
        let qt = self.qt_thread();
        let listing = model.clone();
        model.connect_state_changed(move |state| {
            if *state != LoadState::Ready {
                let text = state.id().to_string();
                let _ = qt.queue(move |obj| obj.set_status(QString::from(text.as_str())));
                return;
            }
            let store = listing.store();
            let mut entries: Vec<FileEntry> = (0..store.n_items())
                .filter_map(|i| store.item(i).and_downcast::<FileEntry>())
                .filter(|e| !e.data().is_hidden)
                .collect();
            entries.sort_by(|a, b| sort::compare(a.data(), b.data(), SortKey::Name, false));
            let names: Vec<String> = entries.iter().map(|e| e.display_name().to_string()).collect();
            let kinds: Vec<String> = entries.iter().map(|e| e.kind().id().to_string()).collect();
            let sizes: Vec<String> = entries.iter().map(|e| e.data().size.to_string()).collect();
            let (names, kinds, sizes) = (names.join("\n"), kinds.join("\n"), sizes.join("\n"));
            let _ = qt.queue(move |mut obj| {
                obj.as_mut().set_kinds(QString::from(kinds.as_str()));
                obj.as_mut().set_sizes(QString::from(sizes.as_str()));
                obj.as_mut().set_names(QString::from(names.as_str()));
                obj.set_status(QString::from("ready"));
            });
        });
        model.load(&gio::File::for_path(path.to_string()));
        MODELS.with(|m| m.borrow_mut().push(model));
        self.as_mut().set_status(QString::from("loading"));
    }
}
