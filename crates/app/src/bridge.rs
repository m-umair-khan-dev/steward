//! The presenter: the only QObject, and the only place that touches Qt types
//! (ADR 0009). It publishes the controller's state as JSON properties and
//! passes QML's commands to the controller. Kept deliberately thin.

#![allow(unsafe_code)] // cxx-qt bridges need `unsafe extern "C++"`.

use std::pin::Pin;

use cxx_qt::Threading;
use cxx_qt_lib::QString;

use crate::controller::{self, Event, dirty};
use crate::{actions, dbus, settings};
use gio::prelude::*;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    // The system clipboard (cpp/clipboard.cpp).
    unsafe extern "C++" {
        include!("cairn/clipboard.h");
        fn clipboard_set(mimes: &Vec<String>, data: &Vec<String>);
        fn clipboard_get(mime: &str) -> String;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, folder)]
        #[qproperty(QString, rows)]
        #[qproperty(QString, items)]
        #[qproperty(QString, selection)]
        #[qproperty(QString, thumbs)]
        #[qproperty(QString, counts)]
        #[qproperty(QString, sidebar)]
        #[qproperty(QString, preview)]
        #[qproperty(QString, pathbar)]
        #[qproperty(QString, nav)]
        #[qproperty(QString, prefs)]
        #[qproperty(QString, accels)]
        #[qproperty(QString, dialog)]
        #[qproperty(QString, dialogdata)]
        #[qproperty(QString, version)]
        #[qproperty(QString, ops)]
        type Presenter = super::PresenterRust;

        #[qsignal]
        fn quitting(self: Pin<&mut Presenter>);
        #[qsignal]
        fn shoot(self: Pin<&mut Presenter>, path: QString);
        #[qsignal]
        fn toast(self: Pin<&mut Presenter>, text: QString, undo: bool);
        #[qsignal]
        fn focus(self: Pin<&mut Presenter>, target: QString);

        #[qinvokable]
        fn activate(self: Pin<&mut Presenter>, action: &QString, param: &QString);
        #[qinvokable]
        fn pick(self: Pin<&mut Presenter>, position: i32, mode: i32);
        #[qinvokable]
        fn open(self: Pin<&mut Presenter>, position: i32);
        #[qinvokable]
        fn arrow(self: Pin<&mut Presenter>, key: &QString, extend: bool);
        #[qinvokable]
        fn filter(self: Pin<&mut Presenter>, text: &QString);
        #[qinvokable]
        fn rename(self: Pin<&mut Presenter>, uri: &QString, name: &QString);
        #[qinvokable]
        fn answer(self: Pin<&mut Presenter>, choice: &QString, all: bool);
        #[qinvokable]
        fn dragged(self: Pin<&mut Presenter>, uri: &QString) -> QString;
        #[qinvokable]
        fn dropped(self: Pin<&mut Presenter>, uris: &QString, target: &QString, ctrl: bool);
        #[qinvokable]
        fn flying(self: Pin<&mut Presenter>, count: i32);
        #[qinvokable]
        fn typed(self: Pin<&mut Presenter>, text: &QString);
        #[qinvokable]
        fn complete(self: Pin<&mut Presenter>, text: &QString) -> QString;
        #[qinvokable]
        fn submit(self: Pin<&mut Presenter>, text: &QString);
        #[qinvokable]
        fn cancelpath(self: Pin<&mut Presenter>);
        #[qinvokable]
        fn resize(self: Pin<&mut Presenter>, width: f64);
        #[qinvokable]
        fn window(self: Pin<&mut Presenter>, width: i32, height: i32, active: bool);
        #[qinvokable]
        fn darkmode(self: Pin<&mut Presenter>, dark: bool);
        #[qinvokable]
        fn card(
            self: Pin<&mut Presenter>,
            name: &QString,
            thumbnail: bool,
            count: &QString,
            delta: i32,
        );
        #[qinvokable]
        fn rowbound(self: Pin<&mut Presenter>, delta: i32);
        #[qinvokable]
        fn seen(self: Pin<&mut Presenter>, uri: &QString, visible: bool);
        #[qinvokable]
        fn shot(self: Pin<&mut Presenter>, path: &QString, ok: bool);
        #[qinvokable]
        fn shown(self: Pin<&mut Presenter>, dialog: &QString);
        #[qinvokable]
        fn launch(self: Pin<&mut Presenter>, index: i32);
        #[qinvokable]
        fn setpref(self: Pin<&mut Presenter>, key: &QString, value: &QString);
        #[qinvokable]
        fn refresh(self: Pin<&mut Presenter>);
    }

    impl cxx_qt::Threading for Presenter {}
    impl cxx_qt::Initialize for Presenter {}
}

#[derive(Default)]
pub struct PresenterRust {
    folder: QString,
    rows: QString,
    items: QString,
    selection: QString,
    thumbs: QString,
    counts: QString,
    sidebar: QString,
    preview: QString,
    pathbar: QString,
    nav: QString,
    prefs: QString,
    accels: QString,
    dialog: QString,
    dialogdata: QString,
    version: QString,
    ops: QString,
}

fn q(text: &str) -> QString {
    QString::from(text)
}

impl cxx_qt::Initialize for qobject::Presenter {
    fn initialize(mut self: Pin<&mut Self>) {
        self.as_mut().set_version(q(env!("CARGO_PKG_VERSION")));
        self.as_mut()
            .set_accels(q(&actions::accels_json().to_string()));
        let qt = self.qt_thread();
        controller::init(move || {
            let _ = qt.queue(|presenter| presenter.refresh());
        });
        dbus::start(|| controller::with(|c| c.quit()));
    }
}

impl qobject::Presenter {
    /// Publishes what changed since the last refresh.
    pub fn refresh(mut self: Pin<&mut Self>) {
        let (updates, events, dialog_data) = controller::with(|c| {
            let (flags, events) = c.take_changes();
            let mut updates: Vec<(u32, String)> = Vec::new();
            let mut push = |flag: u32, value: serde_json::Value| {
                if flags & flag != 0 {
                    updates.push((flag, value.to_string()));
                }
            };
            push(dirty::FOLDER, c.folder_json());
            push(dirty::ROWS, c.rows_json());
            push(dirty::ITEMS, c.items_json());
            push(dirty::SELECTION, c.selection_json());
            push(dirty::THUMBS, c.thumbs_json());
            push(dirty::COUNTS, c.counts_json());
            push(dirty::SIDEBAR, c.sidebar_json());
            push(dirty::PREVIEW, c.preview_json());
            push(dirty::PATH, c.path_json());
            push(dirty::NAV, c.nav_json());
            push(dirty::PREFS, c.prefs_json());
            push(dirty::OPS, c.ops_json());
            (updates, events, c.dialog_data().to_string())
        });
        for (flag, value) in updates {
            let value = q(&value);
            let this = self.as_mut();
            match flag {
                dirty::FOLDER => this.set_folder(value),
                dirty::ROWS => this.set_rows(value),
                dirty::ITEMS => this.set_items(value),
                dirty::SELECTION => this.set_selection(value),
                dirty::THUMBS => this.set_thumbs(value),
                dirty::COUNTS => this.set_counts(value),
                dirty::SIDEBAR => this.set_sidebar(value),
                dirty::PREVIEW => this.set_preview(value),
                dirty::PATH => this.set_pathbar(value),
                dirty::NAV => this.set_nav(value),
                dirty::OPS => this.set_ops(value),
                _ => this.set_prefs(value),
            }
        }
        for event in events {
            match event {
                Event::Toast(text, undo) => self.as_mut().toast(q(&text), undo),
                Event::Quit => self.as_mut().quitting(),
                Event::Screenshot(path) => self.as_mut().shoot(q(&path)),
                Event::Focus(target) => self.as_mut().focus(q(target)),
                Event::Dialog(kind) => {
                    self.as_mut().set_dialogdata(q(&dialog_data));
                    self.as_mut().set_dialog(q(&kind));
                }
            }
        }
    }

    pub fn activate(self: Pin<&mut Self>, action: &QString, param: &QString) {
        actions::activate(&action.to_string(), &param.to_string());
    }

    pub fn pick(self: Pin<&mut Self>, position: i32, mode: i32) {
        controller::with(|c| c.pick(position.max(0) as usize, mode));
    }

    pub fn open(self: Pin<&mut Self>, position: i32) {
        controller::with(|c| c.open_position(position.max(0) as usize));
    }

    pub fn arrow(self: Pin<&mut Self>, key: &QString, extend: bool) {
        controller::with(|c| c.move_selection(&key.to_string(), extend));
    }

    pub fn filter(self: Pin<&mut Self>, text: &QString) {
        controller::with(|c| c.set_name_filter(&text.to_string()));
    }

    pub fn rename(self: Pin<&mut Self>, uri: &QString, name: &QString) {
        controller::with(|c| c.finish_rename(&uri.to_string(), &name.to_string()));
    }

    pub fn answer(self: Pin<&mut Self>, choice: &QString, all: bool) {
        controller::with(|c| c.answer_clash(&choice.to_string(), all));
    }

    pub fn dragged(self: Pin<&mut Self>, uri: &QString) -> QString {
        q(&controller::with(|c| c.drag_uris(&uri.to_string())))
    }

    pub fn dropped(self: Pin<&mut Self>, uris: &QString, target: &QString, ctrl: bool) {
        let (uris, target) = (uris.to_string(), target.to_string());
        controller::with(|c| c.dropped(&uris, &target, ctrl));
    }

    pub fn flying(self: Pin<&mut Self>, count: i32) {
        controller::with(|c| c.set_flying(count));
    }

    pub fn typed(self: Pin<&mut Self>, text: &QString) {
        controller::with(|c| c.path_typed(&text.to_string()));
    }

    pub fn complete(self: Pin<&mut Self>, text: &QString) -> QString {
        q(&controller::with(|c| c.complete_path(&text.to_string())))
    }

    pub fn submit(self: Pin<&mut Self>, text: &QString) {
        controller::with(|c| c.submit_path(&text.to_string()));
    }

    pub fn cancelpath(self: Pin<&mut Self>) {
        controller::with(|c| c.stop_path_edit());
    }

    pub fn resize(self: Pin<&mut Self>, width: f64) {
        controller::with(|c| c.set_width(width));
    }

    pub fn window(self: Pin<&mut Self>, width: i32, height: i32, active: bool) {
        controller::with(|c| c.set_window(width, height, active));
        let s = settings::get();
        if width > 0 && height > 0 {
            let _ = s.set_int("window-width", width);
            let _ = s.set_int("window-height", height);
        }
    }

    pub fn darkmode(self: Pin<&mut Self>, dark: bool) {
        controller::with(|c| c.set_dark(dark));
    }

    pub fn card(
        self: Pin<&mut Self>,
        name: &QString,
        thumbnail: bool,
        count: &QString,
        delta: i32,
    ) {
        controller::with(|c| c.card(&name.to_string(), thumbnail, &count.to_string(), delta));
    }

    pub fn rowbound(self: Pin<&mut Self>, delta: i32) {
        controller::with(|c| c.row_bound(delta));
    }

    pub fn seen(self: Pin<&mut Self>, uri: &QString, visible: bool) {
        controller::with(|c| c.item_visible(&uri.to_string(), visible));
    }

    pub fn shot(self: Pin<&mut Self>, path: &QString, ok: bool) {
        dbus::screenshot_done(&path.to_string(), ok);
    }

    pub fn shown(self: Pin<&mut Self>, dialog: &QString) {
        let kind = dialog.to_string();
        controller::with(|c| {
            if kind.is_empty() {
                c.close_dialog()
            } else {
                c.dialog_shown(&kind)
            }
        });
    }

    pub fn launch(self: Pin<&mut Self>, index: i32) {
        controller::with(|c| c.launch_app(index.max(0) as usize));
    }

    /// Preferences dialog: writes one setting ("true"/"false" for switches).
    pub fn setpref(self: Pin<&mut Self>, key: &QString, value: &QString) {
        let (key, value) = (key.to_string(), value.to_string());
        let s = settings::get();
        let result = match value.as_str() {
            "true" | "false" => s.set_boolean(&key, value == "true"),
            _ => s.set_string(&key, &value),
        };
        if let Err(error) = result {
            glib::g_warning!("cairn", "could not set {key}: {error}");
        }
    }
}
