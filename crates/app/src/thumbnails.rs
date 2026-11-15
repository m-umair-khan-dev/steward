//! Thumbnails (DESIGN.md P1-T08, §8.5).
//!
//! Existing thumbnails come from the shared FreeDesktop cache (GIO reports them
//! as `thumbnail::path` while listing). Missing ones are made by the system's
//! thumbnailers (`/usr/share/thumbnailers/*.thumbnailer`), run inside a
//! bubblewrap sandbox with no network and a read-only view of the one input
//! file (ADR 0005). Only items that are on screen are queued.

use std::cell::{Cell, RefCell};
use std::collections::{HashSet, VecDeque};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{gdk_pixbuf, gio, glib};

use cairn_core::FileEntry;

const MAX_RUNNING: usize = 2;
const SIZE: &str = "256"; // the "large" size in the FreeDesktop spec
const TIMEOUT: Duration = Duration::from_secs(15);

struct Spec {
    mime_types: Vec<String>,
    exec: Vec<OsString>,
}

pub struct Thumbnailer {
    specs: Vec<Spec>,
    sandbox: Option<PathBuf>,
    queue: RefCell<VecDeque<FileEntry>>,
    queued: RefCell<HashSet<String>>,
    failed: RefCell<HashSet<String>>,
    attempts: RefCell<std::collections::HashMap<String, u32>>,
    running: Cell<usize>,
    serial: Cell<u64>,
}

impl Thumbnailer {
    pub fn new() -> Rc<Self> {
        let sandbox = glib::find_program_in_path("bwrap");
        if sandbox.is_none() {
            glib::g_warning!("cairn", "bwrap not found; thumbnails will not be generated");
        }
        Rc::new(Self {
            specs: load_specs(),
            sandbox,
            queue: RefCell::default(),
            queued: RefCell::default(),
            failed: RefCell::default(),
            attempts: RefCell::default(),
            running: Cell::new(0),
            serial: Cell::new(0),
        })
    }

    fn spec_for(&self, content_type: &str) -> Option<&Spec> {
        self.specs.iter().find(|spec| spec.mime_types.iter().any(|m| m == content_type))
    }

    fn wants(&self, entry: &FileEntry) -> bool {
        let data = entry.data();
        !data.is_dir
            && entry.thumbnail_path().is_none()
            && data.file.path().is_some()
            && self.spec_for(&data.content_type).is_some()
            && !self.failed.borrow().contains(&data.uri())
    }

    /// Queues a thumbnail for `entry` if it needs one.
    pub fn request(self: &Rc<Self>, entry: &FileEntry) {
        if !self.wants(entry) || !self.queued.borrow_mut().insert(entry.data().uri()) {
            return;
        }
        self.queue.borrow_mut().push_back(entry.clone());
        self.pump();
    }

    /// Drops `entry` from the queue if it has not started (it scrolled away).
    pub fn cancel(&self, entry: &FileEntry) {
        let mut queue = self.queue.borrow_mut();
        let before = queue.len();
        queue.retain(|queued| queued != entry);
        if queue.len() != before {
            self.queued.borrow_mut().remove(&entry.data().uri());
        }
    }

    fn pump(self: &Rc<Self>) {
        while self.running.get() < MAX_RUNNING {
            let Some(entry) = self.queue.borrow_mut().pop_front() else {
                return;
            };
            self.running.set(self.running.get() + 1);
            let this = self.clone();
            glib::spawn_future_local(async move {
                let uri = entry.data().uri();
                match this.generate(&entry).await {
                    Ok(path) => entry.set_thumbnail_path(Some(&path.to_string_lossy())),
                    Err(reason) => {
                        let attempts = {
                            let mut map = this.attempts.borrow_mut();
                            let n = map.entry(uri.clone()).or_insert(0);
                            *n += 1;
                            *n
                        };
                        glib::g_debug!("cairn", "no thumbnail for {uri} (attempt {attempts}): {reason}");
                        if attempts >= 2 {
                            this.failed.borrow_mut().insert(uri.clone());
                        } else {
                            // Try once more at the back of the queue.
                            this.queue.borrow_mut().push_back(entry.clone());
                            this.running.set(this.running.get() - 1);
                            this.pump();
                            return;
                        }
                    }
                }
                this.queued.borrow_mut().remove(&uri);
                this.running.set(this.running.get() - 1);
                this.pump();
            });
        }
    }

    async fn generate(&self, entry: &FileEntry) -> Result<PathBuf, String> {
        let data = entry.data();
        let input = data.file.path().ok_or("not a local file")?;
        let spec = self.spec_for(&data.content_type).ok_or("no thumbnailer")?;
        let bwrap = self.sandbox.as_ref().ok_or("no sandbox")?;
        let uri = data.uri();
        let md5 = glib::compute_checksum_for_string(glib::ChecksumType::Md5, &uri)
            .ok_or("checksum")?;

        let cache = glib::user_cache_dir().join("thumbnails").join("large");
        create_private_dir(&cache)?;
        self.serial.set(self.serial.get() + 1);
        let work = glib::user_cache_dir()
            .join("cairn")
            .join("thumbnail-work")
            .join(format!("{md5}-{}", self.serial.get()));
        let _ = std::fs::remove_dir_all(&work);
        create_private_dir(&work)?;

        let ext = input.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
        let inner_input = format!("/tmp/input{ext}");
        let argv = sandbox_argv(bwrap, &input, &inner_input, &work, &spec.exec);
        let argv_refs: Vec<&std::ffi::OsStr> = argv.iter().map(OsString::as_os_str).collect();
        let process = gio::Subprocess::newv(
            &argv_refs,
            gio::SubprocessFlags::STDOUT_SILENCE | gio::SubprocessFlags::STDERR_SILENCE,
        )
        .map_err(|e| e.to_string())?;
        match glib::future_with_timeout(TIMEOUT, process.wait_future()).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => return Err(error.to_string()),
            Err(_) => {
                process.force_exit();
                return Err("thumbnailer timed out".into());
            }
        }
        if !process.is_successful() {
            let _ = std::fs::remove_dir_all(&work);
            return Err(format!("thumbnailer exited with {}", process.exit_status()));
        }

        let produced = work.join("thumb.png");
        let pixbuf = gdk_pixbuf::Pixbuf::from_file(&produced).map_err(|e| e.to_string())?;
        let final_path = cache.join(format!("{md5}.png"));
        let temp_path = cache.join(format!("{md5}.png.cairn-part-{}", self.serial.get()));
        let mtime = data.modified.to_string();
        pixbuf
            .savev(
                &temp_path,
                "png",
                &[
                    ("tEXt::Thumb::URI", uri.as_str()),
                    ("tEXt::Thumb::MTime", mtime.as_str()),
                    ("tEXt::Software", "Cairn"),
                ],
            )
            .map_err(|e| e.to_string())?;
        std::fs::rename(&temp_path, &final_path).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_dir_all(&work);
        Ok(final_path)
    }
}

/// The sandbox: read-only /usr, a private /tmp, the input bound read-only, one
/// writable output folder, every namespace unshared (so no network), no /proc.
fn sandbox_argv(
    bwrap: &Path,
    input: &Path,
    inner_input: &str,
    work: &Path,
    exec: &[OsString],
) -> Vec<OsString> {
    let mut argv: Vec<OsString> = vec![bwrap.into()];
    let mut add = |items: &[&str]| argv.extend(items.iter().map(OsString::from));
    add(&["--ro-bind", "/usr", "/usr"]);
    add(&["--symlink", "usr/lib", "/lib", "--symlink", "usr/lib64", "/lib64"]);
    add(&["--symlink", "usr/bin", "/bin", "--symlink", "usr/sbin", "/sbin"]);
    add(&["--ro-bind-try", "/etc/ld.so.cache", "/etc/ld.so.cache"]);
    add(&["--ro-bind-try", "/etc/fonts", "/etc/fonts"]);
    add(&["--dev", "/dev", "--tmpfs", "/tmp"]);
    argv.push("--ro-bind".into());
    argv.push(input.into());
    argv.push(inner_input.into());
    argv.push("--bind".into());
    argv.push(work.into());
    argv.push("/tmp/out".into());
    let mut add = |items: &[&str]| argv.extend(items.iter().map(OsString::from));
    add(&["--unshare-all", "--die-with-parent", "--new-session", "--clearenv"]);
    add(&["--setenv", "PATH", "/usr/bin:/bin", "--setenv", "HOME", "/tmp", "--"]);
    let input_uri = format!("file://{inner_input}");
    for arg in exec {
        let replaced = arg
            .to_string_lossy()
            .replace("%u", &input_uri)
            .replace("%i", inner_input)
            .replace("%o", "/tmp/out/thumb.png")
            .replace("%s", SIZE);
        argv.push(replaced.into());
    }
    argv
}

fn create_private_dir(dir: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    Ok(())
}

fn load_specs() -> Vec<Spec> {
    let mut dirs = vec![glib::user_data_dir()];
    dirs.extend(glib::system_data_dirs());
    let mut specs = Vec::new();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir.join("thumbnailers")) else {
            continue;
        };
        let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        paths.sort();
        for path in paths.into_iter().filter(|p| p.extension().is_some_and(|e| e == "thumbnailer")) {
            if let Some(spec) = parse_spec(&path) {
                specs.push(spec);
            }
        }
    }
    specs
}

fn parse_spec(path: &Path) -> Option<Spec> {
    let file = glib::KeyFile::new();
    file.load_from_file(path, glib::KeyFileFlags::NONE).ok()?;
    let group = "Thumbnailer Entry";
    if let Ok(try_exec) = file.string(group, "TryExec") {
        glib::find_program_in_path(try_exec.as_str())?;
    }
    let exec = glib::shell_parse_argv(file.string(group, "Exec").ok()?.as_str()).ok()?;
    let mime_types = file
        .string_list(group, "MimeType")
        .ok()?
        .iter()
        .map(|m| m.to_string())
        .collect();
    Some(Spec { mime_types, exec })
}
