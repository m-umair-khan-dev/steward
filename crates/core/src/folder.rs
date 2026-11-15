//! Listing one folder into a `gio::ListStore` of [`FileEntry`], and keeping it
//! up to date as files change on disk (DESIGN.md P1-T01, P1-T10).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gio::prelude::*;

use crate::entry::{ATTRIBUTES, FileEntry};

/// Where a listing is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadState {
    Loading,
    Ready,
    /// The folder exists but cannot be read.
    PermissionDenied,
    /// The folder does not exist (any more).
    NotFound,
    /// Not a folder.
    NotAFolder,
    Failed(String),
}

impl LoadState {
    pub fn id(&self) -> &'static str {
        match self {
            LoadState::Loading => "loading",
            LoadState::Ready => "ready",
            LoadState::PermissionDenied => "permission-denied",
            LoadState::NotFound => "not-found",
            LoadState::NotAFolder => "not-a-folder",
            LoadState::Failed(_) => "failed",
        }
    }

    fn from_error(error: &glib::Error) -> Self {
        if error.matches(gio::IOErrorEnum::PermissionDenied) {
            LoadState::PermissionDenied
        } else if error.matches(gio::IOErrorEnum::NotFound) {
            LoadState::NotFound
        } else if error.matches(gio::IOErrorEnum::NotDirectory) {
            LoadState::NotAFolder
        } else {
            LoadState::Failed(error.message().to_string())
        }
    }
}

type StateCallback = Box<dyn Fn(&LoadState)>;

struct Inner {
    store: gio::ListStore,
    folder: RefCell<Option<gio::File>>,
    state: RefCell<LoadState>,
    generation: Cell<u64>,
    monitor: RefCell<Option<gio::FileMonitor>>,
    cancellable: RefCell<Option<gio::Cancellable>>,
    on_state: RefCell<Vec<StateCallback>>,
}

/// A folder's contents. Cheap to clone; clones share the same listing.
#[derive(Clone)]
pub struct FolderModel {
    inner: Rc<Inner>,
}

impl Default for FolderModel {
    fn default() -> Self {
        Self::new()
    }
}

impl FolderModel {
    pub fn new() -> Self {
        Self {
            inner: Rc::new(Inner {
                store: gio::ListStore::new::<FileEntry>(),
                folder: RefCell::new(None),
                state: RefCell::new(LoadState::Ready),
                generation: Cell::new(0),
                monitor: RefCell::new(None),
                cancellable: RefCell::new(None),
                on_state: RefCell::new(Vec::new()),
            }),
        }
    }

    /// The listing. Its items are [`FileEntry`] objects, unsorted.
    pub fn store(&self) -> &gio::ListStore {
        &self.inner.store
    }

    pub fn folder(&self) -> Option<gio::File> {
        self.inner.folder.borrow().clone()
    }

    pub fn state(&self) -> LoadState {
        self.inner.state.borrow().clone()
    }

    /// Calls `callback` whenever the load state changes.
    pub fn connect_state_changed(&self, callback: impl Fn(&LoadState) + 'static) {
        self.inner.on_state.borrow_mut().push(Box::new(callback));
    }

    fn set_state(&self, state: LoadState) {
        *self.inner.state.borrow_mut() = state.clone();
        for callback in self.inner.on_state.borrow().iter() {
            callback(&state);
        }
    }

    /// Starts listing `folder`, replacing whatever was listed before. Earlier
    /// loads that are still running are cancelled and their results dropped.
    pub fn load(&self, folder: &gio::File) {
        if let Some(previous) = self.inner.cancellable.borrow_mut().take() {
            previous.cancel();
        }
        self.inner.monitor.borrow_mut().take();
        let generation = self.inner.generation.get() + 1;
        self.inner.generation.set(generation);
        *self.inner.folder.borrow_mut() = Some(folder.clone());
        self.inner.store.remove_all();
        self.set_state(LoadState::Loading);

        let cancellable = gio::Cancellable::new();
        *self.inner.cancellable.borrow_mut() = Some(cancellable.clone());
        let this = self.clone();
        let folder = folder.clone();
        glib::spawn_future_local(async move {
            let result = list(&folder, &cancellable).await;
            if this.inner.generation.get() != generation {
                return; // superseded by a newer load
            }
            match result {
                Ok(entries) => {
                    this.inner.store.splice(0, 0, &entries);
                    this.watch(&folder, generation);
                    this.set_state(LoadState::Ready);
                }
                Err(error) => this.set_state(LoadState::from_error(&error)),
            }
        });
    }

    /// Lists the folder again.
    pub fn reload(&self) {
        if let Some(folder) = self.folder() {
            self.load(&folder);
        }
    }

    fn watch(&self, folder: &gio::File, generation: u64) {
        let Ok(monitor) =
            folder.monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE)
        else {
            return;
        };
        let this = self.clone();
        let parent = folder.clone();
        monitor.connect_changed(move |_, file, other, event| {
            if this.inner.generation.get() != generation {
                return;
            }
            use gio::FileMonitorEvent as E;
            match event {
                E::Created | E::MovedIn | E::ChangesDoneHint | E::AttributeChanged => {
                    this.refresh_child(&parent, file);
                }
                E::Deleted | E::MovedOut => this.remove_child(file),
                E::Renamed => {
                    this.remove_child(file);
                    if let Some(other) = other {
                        this.refresh_child(&parent, other);
                    }
                }
                E::Unmounted => this.set_state(LoadState::NotFound),
                _ => {}
            }
            if event == E::Deleted && file.equal(&parent) {
                this.set_state(LoadState::NotFound);
            }
        });
        *self.inner.monitor.borrow_mut() = Some(monitor);
    }

    fn position_of(&self, name: &str) -> Option<u32> {
        let store = &self.inner.store;
        (0..store.n_items()).find(|&i| {
            store
                .item(i)
                .and_downcast::<FileEntry>()
                .is_some_and(|entry| entry.name() == name)
        })
    }

    fn remove_child(&self, file: &gio::File) {
        if let Some(name) = file.basename() {
            if let Some(pos) = self.position_of(&name.to_string_lossy()) {
                self.inner.store.remove(pos);
            }
        }
    }

    fn refresh_child(&self, parent: &gio::File, file: &gio::File) {
        let Some(parent_of_file) = file.parent() else {
            return;
        };
        if !parent_of_file.equal(parent) {
            return;
        }
        let this = self.clone();
        let parent = parent.clone();
        let file = file.clone();
        glib::spawn_future_local(async move {
            let Ok(info) = file
                .query_info_future(ATTRIBUTES, gio::FileQueryInfoFlags::NONE, glib::Priority::DEFAULT)
                .await
            else {
                return;
            };
            let entry = FileEntry::from_info(&parent, &info);
            match this.position_of(entry.name()) {
                Some(pos) => this.inner.store.splice(pos, 1, &[entry]),
                None => this.inner.store.append(&entry),
            }
        });
    }
}

async fn list(folder: &gio::File, cancellable: &gio::Cancellable) -> Result<Vec<FileEntry>, glib::Error> {
    let enumerator = folder
        .enumerate_children_future(ATTRIBUTES, gio::FileQueryInfoFlags::NONE, glib::Priority::DEFAULT)
        .await?;
    let mut entries = Vec::new();
    loop {
        if cancellable.is_cancelled() {
            return Err(glib::Error::new(gio::IOErrorEnum::Cancelled, "cancelled"));
        }
        let infos = enumerator.next_files_future(500, glib::Priority::DEFAULT).await?;
        if infos.is_empty() {
            break;
        }
        entries.extend(infos.iter().map(|info| FileEntry::from_info(folder, info)));
    }
    Ok(entries)
}
