//! cairn-indexer: keeps `index.db` up to date (DESIGN.md §7.1, Phase 3).
//!
//! - Crawls the roots at idle priority, then watches every folder (GIO file
//!   monitors, i.e. inotify). Past the watch limit it re-crawls on a timer.
//! - Is the only writer of the index; the app reads it.
//! - D-Bus `dev.cairn.Indexer1` at `/dev/cairn/Indexer`: Status, Pause,
//!   Resume, Rescan, WaitIdle, Notify (the app's own changes), and the
//!   signal Changed(folders).
//! - Reads the same settings as the app (roots, exclusions, pause, pause on
//!   battery). Started by the app in development; a systemd user service and
//!   D-Bus activation once installed.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gio::prelude::*;

use cairn_core::index::{Index, Rules};

const BUS_NAME: &str = "dev.cairn.Indexer";
const OBJECT_PATH: &str = "/dev/cairn/Indexer";
const INTERFACE: &str = r#"
<node>
  <interface name="dev.cairn.Indexer1">
    <method name="Status"><arg type="s" name="json" direction="out"/></method>
    <method name="Pause"/>
    <method name="Resume"/>
    <method name="Rescan"/>
    <method name="WaitIdle">
      <arg type="u" name="timeout_ms" direction="in"/><arg type="b" name="idle" direction="out"/>
    </method>
    <method name="Notify"><arg type="as" name="paths" direction="in"/></method>
    <signal name="Changed"><arg type="as" name="folders"/></signal>
  </interface>
</node>"#;

/// Folders changed in a burst are refreshed together after this pause.
const SETTLE: Duration = Duration::from_millis(150);
/// Past the watch limit, everything is re-crawled this often.
const POLL_EVERY: Duration = Duration::from_secs(15 * 60);

enum Work {
    Crawl,
    Refresh(PathBuf),
}

/// What the worker shares with the main thread.
struct Shared {
    state: Mutex<&'static str>,
    counts: Mutex<(u64, u64)>,
    complete: AtomicBool,
    /// Paused by the user (setting) or by the battery.
    paused: AtomicBool,
    /// Work queued or running.
    busy: AtomicUsize,
    /// A crawl stopped by Pause, to finish on Resume.
    crawl_owed: AtomicBool,
}

struct Main {
    shared: Arc<Shared>,
    jobs: Sender<Work>,
    connection: Option<gio::DBusConnection>,
    monitors: HashMap<PathBuf, gio::FileMonitor>,
    watch_limit: usize,
    polling: bool,
    pending: HashSet<PathBuf>,
    settle: Option<glib::SourceId>,
    settings: gio::Settings,
    on_battery: bool,
}

thread_local! {
    static MAIN: RefCell<Option<Main>> = const { RefCell::new(None) };
}

fn with<R>(f: impl FnOnce(&mut Main) -> R) -> Option<R> {
    MAIN.with(|cell| cell.borrow_mut().as_mut().map(f))
}

fn settings() -> gio::Settings {
    #[cfg(any(debug_assertions, feature = "e2e"))]
    {
        let dir = concat!(env!("OUT_DIR"), "/schemas");
        let parent = gio::SettingsSchemaSource::default();
        if let Ok(source) = gio::SettingsSchemaSource::from_directory(dir, parent.as_ref(), false)
            && let Some(schema) = source.lookup(cairn_core::APP_ID, false)
        {
            return gio::Settings::new_full(&schema, None::<&gio::SettingsBackend>, None);
        }
    }
    gio::Settings::new(cairn_core::APP_ID)
}

fn roots_and_rules(settings: &gio::Settings) -> (Vec<PathBuf>, Rules) {
    let home = glib::home_dir();
    let mut roots: Vec<PathBuf> = settings
        .strv("index-roots")
        .iter()
        .map(|r| PathBuf::from(r.as_str()))
        .collect();
    if roots.is_empty() {
        roots.push(home.clone());
    }
    let mut rules = Rules::for_home(&home);
    rules.exclude.extend(
        settings
            .strv("index-exclude")
            .iter()
            .map(|r| PathBuf::from(r.as_str())),
    );
    (roots, rules)
}

// --- the worker -------------------------------------------------------------------------

fn worker(
    mut index: Index,
    jobs: Receiver<Work>,
    shared: Arc<Shared>,
    roots: Vec<PathBuf>,
    rules: Rules,
) {
    let set_state = |s: &'static str| {
        if let Ok(mut state) = shared.state.lock() {
            *state = s;
        }
    };
    for job in jobs {
        while shared.paused.load(Ordering::SeqCst) {
            set_state("paused");
            std::thread::sleep(Duration::from_millis(100));
        }
        match job {
            Work::Crawl => {
                set_state("crawling");
                let keep_going = || !shared.paused.load(Ordering::SeqCst);
                let mut finished = true;
                for root in &roots {
                    match index.crawl(root, &rules, &keep_going) {
                        Ok(stats) => finished &= stats.finished,
                        Err(error) => {
                            eprintln!("cairn-indexer: crawl of {} failed: {error}", root.display())
                        }
                    }
                }
                shared.crawl_owed.store(!finished, Ordering::SeqCst);
                shared.complete.store(index.is_complete(), Ordering::SeqCst);
                let folders: Vec<PathBuf> =
                    roots.iter().flat_map(|r| index.folders_under(r)).collect();
                after_work(&index, &shared, folders, vec!["*".to_string()]);
            }
            Work::Refresh(dir) => {
                set_state("updating");
                if let Err(error) = index.refresh_dir(&dir, &rules) {
                    eprintln!(
                        "cairn-indexer: refresh of {} failed: {error}",
                        dir.display()
                    );
                }
                let folders = index.folders_under(&dir);
                after_work(
                    &index,
                    &shared,
                    folders,
                    vec![dir.to_string_lossy().into_owned()],
                );
            }
        }
        shared.busy.fetch_sub(1, Ordering::SeqCst);
        if shared.busy.load(Ordering::SeqCst) == 0 {
            set_state(if shared.paused.load(Ordering::SeqCst) {
                "paused"
            } else {
                "idle"
            });
        }
    }
}

/// Publishes counts, asks the main thread to watch `folders`, and announces
/// the change.
fn after_work(index: &Index, shared: &Shared, folders: Vec<PathBuf>, changed: Vec<String>) {
    if let Ok(mut counts) = shared.counts.lock() {
        *counts = index.counts();
    }
    glib::MainContext::default().invoke(move || {
        with(|main| {
            main.watch(&folders);
            main.emit_changed(&changed);
        });
    });
}

// --- the main thread ----------------------------------------------------------------------

impl Main {
    fn queue(&mut self, work: Work) {
        self.shared.busy.fetch_add(1, Ordering::SeqCst);
        let _ = self.jobs.send(work);
    }

    fn effective_pause(&mut self) {
        let paused = self.settings.boolean("index-paused")
            || (self.on_battery && self.settings.boolean("index-pause-on-battery"));
        let was = self.shared.paused.swap(paused, Ordering::SeqCst);
        if was && !paused && self.shared.crawl_owed.swap(false, Ordering::SeqCst) {
            self.queue(Work::Crawl);
        }
    }

    fn watch(&mut self, folders: &[PathBuf]) {
        for folder in folders {
            if self.monitors.contains_key(folder) {
                continue;
            }
            if self.monitors.len() >= self.watch_limit {
                if !self.polling {
                    self.polling = true;
                    eprintln!("cairn-indexer: watch limit reached; re-crawling every 15 minutes");
                    glib::timeout_add_local(POLL_EVERY, || {
                        with(|main| main.queue(Work::Crawl));
                        glib::ControlFlow::Continue
                    });
                }
                return;
            }
            let Ok(monitor) = gio::File::for_path(folder)
                .monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE)
            else {
                continue;
            };
            let dir = folder.clone();
            monitor.connect_changed(move |_, file, other, event| {
                let mut touched = vec![dir.clone()];
                use gio::FileMonitorEvent as E;
                match event {
                    E::Deleted
                    | E::MovedOut
                    | E::Renamed
                    | E::MovedIn
                    | E::Created
                    | E::Changed
                    | E::AttributeChanged
                    | E::ChangesDoneHint => {}
                    _ => return,
                }
                // A folder that went away (or came in) is handled through its parent.
                if let Some(path) = file.path().filter(|p| *p == dir)
                    && let Some(parent) = path.parent()
                {
                    touched.push(parent.to_path_buf());
                }
                if let Some(path) = other
                    .and_then(|o| o.path())
                    .and_then(|p| p.parent().map(Path::to_path_buf))
                {
                    touched.push(path);
                }
                with(|main| main.touch(touched));
            });
            self.monitors.insert(folder.clone(), monitor);
        }
        // Forget watches on folders that are gone.
        self.monitors.retain(|path, monitor| {
            let keep = path.exists();
            if !keep {
                monitor.cancel();
            }
            keep
        });
    }

    /// Folders changed: refresh them once things settle.
    fn touch(&mut self, folders: Vec<PathBuf>) {
        self.pending.extend(folders);
        if let Some(source) = self.settle.take() {
            source.remove();
        }
        self.settle = Some(glib::timeout_add_local_once(SETTLE, || {
            with(|main| {
                main.settle = None;
                let pending: Vec<PathBuf> = main.pending.drain().collect();
                for dir in pending {
                    main.queue(Work::Refresh(dir));
                }
            });
        }));
    }

    fn emit_changed(&self, folders: &[String]) {
        if let Some(connection) = &self.connection {
            let _ = connection.emit_signal(
                None::<&str>,
                OBJECT_PATH,
                "dev.cairn.Indexer1",
                "Changed",
                Some(&(folders.to_vec(),).to_variant()),
            );
        }
    }

    fn idle(&self) -> bool {
        self.shared.busy.load(Ordering::SeqCst) == 0
            && self.pending.is_empty()
            && self.settle.is_none()
    }

    fn status(&self) -> String {
        let (entries, files) = self.shared.counts.lock().map(|c| *c).unwrap_or((0, 0));
        let state = self.shared.state.lock().map(|s| *s).unwrap_or("idle");
        serde_json::json!({
            "state": if self.shared.paused.load(Ordering::SeqCst) { "paused" } else { state },
            "paused": self.shared.paused.load(Ordering::SeqCst),
            "idle": self.idle(),
            "complete": self.shared.complete.load(Ordering::SeqCst),
            "entries": entries,
            "files": files,
            "watching": self.monitors.len(),
            "polling": self.polling,
            "onBattery": self.on_battery,
            "pid": std::process::id(),
        })
        .to_string()
    }
}

fn register(connection: &gio::DBusConnection) {
    let node = gio::DBusNodeInfo::for_xml(INTERFACE).expect("valid interface XML");
    let interface = node
        .lookup_interface("dev.cairn.Indexer1")
        .expect("interface");
    let result = connection
        .register_object(OBJECT_PATH, &interface)
        .method_call(|_, _, _, _, method, params, invocation| match method {
            "Status" => {
                let json = with(|m| m.status()).unwrap_or_default();
                invocation.return_value(Some(&(json,).to_variant()));
            }
            "Pause" | "Resume" => {
                if let Some(settings) = with(|m| m.settings.clone()) {
                    let _ = settings.set_boolean("index-paused", method == "Pause");
                }
                with(|m| m.effective_pause());
                invocation.return_value(None);
            }
            "Rescan" => {
                with(|m| m.queue(Work::Crawl));
                invocation.return_value(None);
            }
            "Notify" => {
                let paths: Vec<String> = params.child_get(0);
                let mut folders = Vec::new();
                for p in paths {
                    let path = PathBuf::from(p);
                    if let Some(parent) = path.parent() {
                        folders.push(parent.to_path_buf());
                    }
                    folders.push(path);
                }
                with(|m| m.touch(folders));
                invocation.return_value(None);
            }
            "WaitIdle" => {
                let timeout: u32 = params.child_get(0);
                let deadline = Instant::now() + Duration::from_millis(u64::from(timeout));
                glib::timeout_add_local(Duration::from_millis(20), move || {
                    let idle = with(|m| m.idle()).unwrap_or(true);
                    if !idle && Instant::now() < deadline {
                        return glib::ControlFlow::Continue;
                    }
                    invocation.clone().return_value(Some(&(idle,).to_variant()));
                    glib::ControlFlow::Break
                });
            }
            _ => invocation.return_dbus_error("org.freedesktop.DBus.Error.UnknownMethod", method),
        })
        .build();
    if let Err(error) = result {
        eprintln!("cairn-indexer: could not register {OBJECT_PATH}: {error}");
    }
}

/// Follows UPower's OnBattery on the system bus, if there is one.
fn watch_battery() {
    gio::DBusProxy::for_bus(
        gio::BusType::System,
        gio::DBusProxyFlags::NONE,
        None,
        "org.freedesktop.UPower",
        "/org/freedesktop/UPower",
        "org.freedesktop.UPower",
        gio::Cancellable::NONE,
        |result| {
            let Ok(proxy) = result else { return };
            let read = |proxy: &gio::DBusProxy| {
                proxy
                    .cached_property("OnBattery")
                    .and_then(|v| v.get::<bool>())
                    .unwrap_or(false)
            };
            let on_battery = read(&proxy);
            with(|m| {
                m.on_battery = on_battery;
                m.effective_pause();
            });
            proxy.connect_g_properties_changed(move |proxy, _, _| {
                let on_battery = read(proxy);
                with(|m| {
                    m.on_battery = on_battery;
                    m.effective_pause();
                });
            });
        },
    );
}

/// About half the user's inotify watches, leaving the rest to other apps.
fn watch_limit() -> usize {
    std::fs::read_to_string("/proc/sys/fs/inotify/max_user_watches")
        .ok()
        .and_then(|s| s.trim().parse::<usize>().ok())
        .map(|n| n / 2)
        .unwrap_or(4096)
        .clamp(256, 200_000)
}

fn main() -> ExitCode {
    // Same rule as the app (AGENTS.md rule 4): development and test builds
    // only ever run against a fixture home.
    let guard_required =
        cairn_core::guard::is_required(cfg!(debug_assertions), cfg!(feature = "e2e"), |name| {
            std::env::var(name).ok()
        });
    if guard_required {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        if let Err(error) = cairn_core::guard::check(home.as_deref()) {
            eprintln!("cairn-indexer: {error}");
            return ExitCode::from(2);
        }
    }
    // When the app started us, go when it goes.
    if std::env::var_os("CAIRN_INDEXER_CHILD").is_some() {
        let _ =
            rustix::process::set_parent_process_death_signal(Some(rustix::process::Signal::TERM));
    }
    // Idle priority: indexing never competes with what the user is doing.
    let _ = rustix::process::nice(19);

    let path = std::env::var_os("CAIRN_INDEX_DB")
        .map(PathBuf::from)
        .unwrap_or_else(Index::default_path);
    let index = match Index::open(&path) {
        Ok(index) => index,
        Err(error) => {
            // A damaged index is only a cache: start again.
            eprintln!("cairn-indexer: {error}; rebuilding {}", path.display());
            for suffix in ["", "-wal", "-shm"] {
                let _ = std::fs::remove_file(format!("{}{suffix}", path.display())); // E2-13 allow: the index's own files
            }
            match Index::open(&path) {
                Ok(index) => index,
                Err(error) => {
                    eprintln!("cairn-indexer: cannot open {}: {error}", path.display());
                    return ExitCode::FAILURE;
                }
            }
        }
    };
    let settings = settings();
    let (roots, rules) = roots_and_rules(&settings);
    let shared = Arc::new(Shared {
        state: Mutex::new("starting"),
        counts: Mutex::new(index.counts()),
        complete: AtomicBool::new(index.is_complete()),
        paused: AtomicBool::new(false),
        busy: AtomicUsize::new(0),
        crawl_owed: AtomicBool::new(false),
    });
    let (jobs, queue) = channel();
    {
        let shared = shared.clone();
        std::thread::Builder::new()
            .name("cairn-index".into())
            .spawn(move || worker(index, queue, shared, roots, rules))
            .expect("start the worker");
    }

    let main_loop = glib::MainLoop::new(None, false);
    MAIN.with(|cell| {
        *cell.borrow_mut() = Some(Main {
            shared,
            jobs,
            connection: None,
            monitors: HashMap::new(),
            watch_limit: watch_limit(),
            polling: false,
            pending: HashSet::new(),
            settle: None,
            settings: settings.clone(),
            on_battery: false,
        })
    });
    // Settings can change inside one of our own calls (Pause sets one), so
    // the reaction waits for the next main-loop turn.
    settings.connect_changed(None, |settings, key| match key {
        "index-paused" | "index-pause-on-battery" => {
            glib::idle_add_local_once(|| {
                with(|m| m.effective_pause());
            });
        }
        // New roots or exclusions: start again with them (the app, or
        // systemd, starts the indexer again when it is needed).
        "index-roots" | "index-exclude" => {
            let _ = settings;
            eprintln!("cairn-indexer: roots or exclusions changed; restarting");
            std::process::exit(0);
        }
        _ => {}
    });
    with(|m| {
        m.effective_pause();
        // Every start checks the whole index against the disk.
        m.queue(Work::Crawl);
    });
    watch_battery();

    let quit = main_loop.clone();
    let _owner = gio::bus_own_name(
        gio::BusType::Session,
        BUS_NAME,
        gio::BusNameOwnerFlags::NONE,
        |connection, _| {
            register(&connection);
            with(|m| m.connection = Some(connection));
        },
        |_, _| {},
        move |_, _| {
            // Another indexer already runs (or the bus went away).
            eprintln!("cairn-indexer: {BUS_NAME} is taken; leaving");
            quit.quit();
        },
    );
    main_loop.run();
    ExitCode::SUCCESS
}
