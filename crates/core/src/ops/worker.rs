//! Runs one request at a time: plans its steps, records them in the journal,
//! performs them, and undoes or redoes earlier batches. Synchronous; the
//! engine runs it on its own thread, and tests call it directly.

use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};

use gio::prelude::*;

use super::fsx;
use super::journal::{Action, Step};
use super::trash::Trash;
use super::{Answer, Choice, Failure, Outcome, Report, Request};
use crate::store::Store;

/// What the worker needs from whoever runs it.
pub trait Context {
    /// Bytes and items done so far out of the totals, and the current name.
    fn progress(
        &mut self,
        bytes: u64,
        total_bytes: u64,
        items: u64,
        total_items: u64,
        current: &str,
    );
    /// `name` already exists in `folder`: what should happen?
    fn ask(&mut self, name: &str, folder: &Path) -> Answer;
    fn cancellable(&self) -> &gio::Cancellable;
}

pub struct Worker {
    pub store: Store,
    pub trash: Trash,
    /// Test hook: a pause before each item and each progress step (see
    /// `Engine::start`).
    pub step_delay: Option<std::time::Duration>,
}

struct Run<'a> {
    worker: &'a Worker,
    ctx: &'a mut dyn Context,
    batch: i64,
    seq: i64,
    bytes: u64,
    total_bytes: u64,
    items: u64,
    total_items: u64,
    report: Report,
    remembered: Option<Choice>,
}

impl Run<'_> {
    fn tick(&mut self, delta: u64, current: &str) {
        if let Some(delay) = self.worker.step_delay {
            std::thread::sleep(delay);
        }
        self.bytes += delta;
        self.ctx.progress(
            self.bytes,
            self.total_bytes,
            self.items,
            self.total_items,
            current,
        );
    }

    fn record(
        &mut self,
        action: Action,
        src: Option<&Path>,
        dst: Option<&Path>,
    ) -> io::Result<i64> {
        self.seq += 1;
        self.worker
            .store
            .add_step(self.batch, self.seq, action, src, dst)
            .map_err(io::Error::other)
    }

    fn done(&mut self, step: i64, dst: Option<&Path>, trash: Option<&Path>) {
        let _ = self.worker.store.set_step(step, "done", dst, trash, None);
        self.report.done += 1;
    }

    fn fail(&mut self, step: Option<i64>, path: &Path, error: &io::Error) {
        if let Some(step) = step {
            let _ =
                self.worker
                    .store
                    .set_step(step, "failed", None, None, Some(&error.to_string()));
        }
        if fsx::is_cancelled(error) {
            self.report.cancelled = true;
        } else {
            self.report.failed.push(Failure::new(path, error));
        }
    }

    fn cancelled(&self) -> bool {
        self.ctx.cancellable().is_cancelled()
    }

    /// Decides what to do when `name` exists in `folder`.
    fn clash(&mut self, name: &str, folder: &Path) -> Choice {
        if let Some(choice) = self.remembered {
            return choice;
        }
        let answer = self.ctx.ask(name, folder);
        if answer.apply_to_all {
            self.remembered = Some(answer.choice);
        }
        answer.choice
    }

    /// Sends what is in the way to the Trash (Replace), as an undoable step.
    fn clear_way(&mut self, target: &Path) -> io::Result<()> {
        let step = self.record(Action::Trash, Some(target), None)?;
        match self.worker.trash.trash(target) {
            Ok(trashed) => {
                let _ = self
                    .worker
                    .store
                    .set_step(step, "done", None, Some(&trashed), None);
                Ok(())
            }
            Err(error) => {
                let _ = self.worker.store.set_step(
                    step,
                    "failed",
                    None,
                    None,
                    Some(&error.to_string()),
                );
                Err(error)
            }
        }
    }

    /// Copy or move each source into `dest` with the clash rules.
    fn transfer(&mut self, sources: &[PathBuf], dest: &Path, moving: bool) {
        for src in sources {
            if self.cancelled() {
                self.report.cancelled = true;
                return;
            }
            if let Some(delay) = self.worker.step_delay {
                std::thread::sleep(delay);
            }
            let name = fsx::file_name(src);
            let mut target = dest.join(&name);
            if moving && src.parent() == Some(dest) {
                continue; // already here
            }
            if dest.starts_with(src) {
                let error = io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "a folder cannot go inside itself",
                );
                self.fail(None, src, &error);
                continue;
            }
            if fsx::exists(&target) {
                // Pasting a copy into its own folder always keeps both.
                let choice = if !moving && src.parent() == Some(dest) {
                    Choice::KeepBoth
                } else {
                    self.clash(&name, dest)
                };
                match choice {
                    Choice::Skip => {
                        self.report.skipped += 1;
                        continue;
                    }
                    Choice::KeepBoth => target = dest.join(fsx::free_name(dest, &name)),
                    Choice::Replace => {
                        if let Err(error) = self.clear_way(&target) {
                            self.fail(None, src, &error);
                            continue;
                        }
                    }
                }
            }
            let action = if moving { Action::Move } else { Action::Copy };
            let step = match self.record(action, Some(src), Some(&target)) {
                Ok(step) => step,
                Err(error) => {
                    self.fail(None, src, &error);
                    continue;
                }
            };
            let cancel = self.ctx.cancellable().clone();
            let label = name.clone();
            let result = if moving {
                fsx::move_path(src, &target, &cancel, &mut |d| self.tick(d, &label))
            } else {
                fsx::copy_tree(src, &target, &cancel, &mut |d| self.tick(d, &label)).map_err(
                    |failure| {
                        // Never leave part of a copy behind.
                        if failure.created {
                            let _ = fsx::remove_tree(&target);
                        }
                        failure.error
                    },
                )
            };
            self.items += 1;
            match result {
                Ok(()) => {
                    self.report.created.push(target.clone());
                    self.done(step, Some(&target), None);
                }
                Err(error) => self.fail(Some(step), src, &error),
            }
        }
    }
}

impl Worker {
    pub fn new(store: Store, trash: Trash) -> Self {
        Self {
            store,
            trash,
            step_delay: None,
        }
    }

    pub fn run(&self, request: &Request, summary: &str, ctx: &mut dyn Context) -> Report {
        match request {
            Request::Undo => self.undo(ctx),
            Request::Redo => self.redo(ctx),
            _ => self.forward(request, summary, ctx),
        }
    }

    fn forward(&self, request: &Request, summary: &str, ctx: &mut dyn Context) -> Report {
        let kind = request.kind();
        let batch = if request.undoable() {
            match self.store.begin_batch(kind, summary) {
                Ok(id) => Some(id),
                Err(error) => return Report::error(kind, io::Error::other(error)),
            }
        } else {
            None
        };
        let (total_bytes, total_items) = request
            .sources()
            .iter()
            .map(|s| fsx::measure(s))
            .fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
        let mut run = Run {
            worker: self,
            ctx,
            batch: batch.unwrap_or(0),
            seq: 0,
            bytes: 0,
            total_bytes,
            items: 0,
            total_items,
            report: Report::new(kind, batch),
            remembered: None,
        };
        match request {
            Request::Copy { sources, dest } => run.transfer(sources, dest, false),
            Request::Move { sources, dest } => run.transfer(sources, dest, true),
            Request::Duplicate { sources } => {
                for src in sources {
                    let Some(dir) = src.parent() else { continue };
                    run.transfer(std::slice::from_ref(src), dir, false);
                }
            }
            Request::Rename { source, name } => {
                let result = source
                    .parent()
                    .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))
                    .and_then(|dir| fsx::child(dir, name));
                match result {
                    Ok(target) if target == *source => {}
                    Ok(target) => match run.record(Action::Rename, Some(source), Some(&target)) {
                        Ok(step) => match fsx::rename_noreplace(source, &target) {
                            Ok(()) => {
                                run.report.created.push(target.clone());
                                run.done(step, Some(&target), None);
                            }
                            Err(error) => run.fail(Some(step), source, &error),
                        },
                        Err(error) => run.fail(None, source, &error),
                    },
                    Err(error) => run.fail(None, source, &error),
                }
            }
            Request::NewFolder { parent, name } => {
                let name = fsx::free_name(parent, name);
                match fsx::child(parent, &name) {
                    Ok(target) => match run.record(Action::Mkdir, None, Some(&target)) {
                        Ok(step) => match fsx::create_dir(&target) {
                            Ok(()) => {
                                run.report.created.push(target.clone());
                                run.done(step, Some(&target), None);
                            }
                            Err(error) => run.fail(Some(step), &target, &error),
                        },
                        Err(error) => run.fail(None, &target, &error),
                    },
                    Err(error) => run.fail(None, parent, &error),
                }
            }
            Request::Trash { sources } => {
                for src in sources {
                    if run.cancelled() {
                        run.report.cancelled = true;
                        break;
                    }
                    let step = match run.record(Action::Trash, Some(src), None) {
                        Ok(step) => step,
                        Err(error) => {
                            run.fail(None, src, &error);
                            continue;
                        }
                    };
                    match self.trash.trash(src) {
                        Ok(trashed) => run.done(step, None, Some(&trashed)),
                        Err(error) => run.fail(Some(step), src, &error),
                    }
                    run.items += 1;
                    run.tick(0, &fsx::file_name(src));
                }
            }
            Request::Restore { items } => {
                for item in items {
                    let Some(original) = self.trash.original_of(item) else {
                        run.fail(None, item, &io::Error::from(io::ErrorKind::NotFound));
                        continue;
                    };
                    let step = match run.record(Action::Restore, Some(item), Some(&original)) {
                        Ok(step) => step,
                        Err(error) => {
                            run.fail(None, item, &error);
                            continue;
                        }
                    };
                    match self.trash.restore(item, &original) {
                        Ok(()) => {
                            run.report.created.push(original.clone());
                            run.done(step, Some(&original), None);
                        }
                        Err(error) => run.fail(Some(step), &original, &error),
                    }
                }
            }
            Request::EmptyTrash => {
                let items: Vec<PathBuf> = self.trash.list().into_iter().map(|i| i.path).collect();
                let mut erased = Vec::new();
                for item in &items {
                    match self.trash.erase(item) {
                        Ok(()) => {
                            run.report.done += 1;
                            erased.push(item.clone());
                        }
                        Err(error) => run.fail(None, item, &error),
                    }
                }
                let _ = self.store.forget_trashed(&erased);
            }
            Request::Undo | Request::Redo => unreachable!(),
        }
        let report = run.report;
        if let Some(batch) = batch {
            if report.done == 0 {
                // Nothing happened (cancelled before the first step, skipped,
                // or every step failed): nothing to undo.
                let _ = self.store.delete_batch(batch);
            } else {
                let state = if report.failed.is_empty() && !report.cancelled {
                    "done"
                } else {
                    "partial"
                };
                let _ = self.store.finish_batch(batch, state);
            }
        }
        report.finish(Outcome::Forward)
    }

    fn undo(&self, ctx: &mut dyn Context) -> Report {
        let batch = match self.store.undo_candidate() {
            Ok(Some(batch)) => batch,
            Ok(None) => return Report::new("undo", None).finish(Outcome::Undo),
            Err(error) => return Report::error("undo", io::Error::other(error)),
        };
        let mut report = Report::new(&batch.kind, Some(batch.id));
        report.summary = batch.summary.clone();
        let steps = self.store.steps(batch.id).unwrap_or_default();
        for step in steps.iter().rev().filter(|s| s.state == "done") {
            ctx.progress(0, 0, report.done as u64, steps.len() as u64, "");
            match self.revert(step) {
                Ok(trashed) => {
                    let _ = self
                        .store
                        .set_step(step.id, "undone", None, trashed.as_deref(), None);
                    if let Some(src) = &step.src
                        && step.action != Action::Copy
                    {
                        report.created.push(src.clone());
                    }
                    report.done += 1;
                }
                Err(error) => {
                    let place = step.src.clone().or(step.dst.clone()).unwrap_or_default();
                    report.failed.push(Failure::new(&place, &error));
                }
            }
        }
        let state = if report.failed.is_empty() {
            "undone"
        } else {
            "partial"
        };
        let _ = self.store.finish_batch(batch.id, state);
        report.finish(Outcome::Undo)
    }

    /// Reverses one done step. Never replaces anything.
    fn revert(&self, step: &Step) -> io::Result<Option<PathBuf>> {
        let need = |p: &Option<PathBuf>| {
            p.clone()
                .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
        };
        match step.action {
            Action::Copy => {
                // Copies go to the Trash, not away for good: Undo never loses data.
                let dst = need(&step.dst)?;
                Ok(Some(self.trash.trash(&dst)?))
            }
            Action::Move | Action::Rename => {
                let (src, dst) = (need(&step.src)?, need(&step.dst)?);
                if !fsx::exists(&dst) {
                    return Err(io::Error::new(
                        io::ErrorKind::NotFound,
                        "it is no longer there",
                    ));
                }
                if fsx::exists(&src) {
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        "the name is taken",
                    ));
                }
                fsx::move_path(&dst, &src, &gio::Cancellable::new(), &mut |_| {})?;
                Ok(None)
            }
            Action::Mkdir => {
                fsx::remove_empty_dir(&need(&step.dst)?)?;
                Ok(None)
            }
            Action::Trash => {
                let (src, item) = (need(&step.src)?, need(&step.trash)?);
                if fsx::exists(&src) {
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        "the name is taken",
                    ));
                }
                self.trash.restore(&item, &src)?;
                Ok(None)
            }
            Action::Restore => Ok(Some(self.trash.trash(&need(&step.dst)?)?)),
            Action::Erase => Err(io::Error::other("cannot be undone")),
        }
    }

    fn redo(&self, ctx: &mut dyn Context) -> Report {
        let batch = match self.store.redo_candidate() {
            Ok(Some(batch)) => batch,
            Ok(None) => return Report::new("redo", None).finish(Outcome::Redo),
            Err(error) => return Report::error("redo", io::Error::other(error)),
        };
        let mut report = Report::new(&batch.kind, Some(batch.id));
        report.summary = batch.summary.clone();
        let steps = self.store.steps(batch.id).unwrap_or_default();
        let cancel = ctx.cancellable().clone();
        for step in steps.iter().filter(|s| s.state == "undone") {
            let result = self.repeat(step, &cancel);
            match result {
                Ok(trashed) => {
                    let _ = self
                        .store
                        .set_step(step.id, "done", None, trashed.as_deref(), None);
                    if let Some(dst) = &step.dst {
                        report.created.push(dst.clone());
                    }
                    report.done += 1;
                }
                Err(error) => {
                    let place = step.dst.clone().or(step.src.clone()).unwrap_or_default();
                    report.failed.push(Failure::new(&place, &error));
                }
            }
        }
        let state = if report.failed.is_empty() {
            "done"
        } else {
            "partial"
        };
        let _ = self.store.finish_batch(batch.id, state);
        report.finish(Outcome::Redo)
    }

    /// Does an undone step again.
    fn repeat(&self, step: &Step, cancel: &gio::Cancellable) -> io::Result<Option<PathBuf>> {
        let need = |p: &Option<PathBuf>| {
            p.clone()
                .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
        };
        match step.action {
            Action::Copy => {
                let (src, dst) = (need(&step.src)?, need(&step.dst)?);
                fsx::copy_tree(&src, &dst, cancel, &mut |_| {}).map_err(|failure| {
                    if failure.created {
                        let _ = fsx::remove_tree(&dst);
                    }
                    failure.error
                })?;
                Ok(None)
            }
            Action::Move | Action::Rename => {
                let (src, dst) = (need(&step.src)?, need(&step.dst)?);
                fsx::move_path(&src, &dst, cancel, &mut |_| {})?;
                Ok(None)
            }
            Action::Mkdir => {
                fsx::create_dir(&need(&step.dst)?)?;
                Ok(None)
            }
            Action::Trash => Ok(Some(self.trash.trash(&need(&step.src)?)?)),
            Action::Restore => {
                let (item, dst) = (need(&step.trash)?, need(&step.dst)?);
                self.trash.restore(&item, &dst)?;
                Ok(None)
            }
            Action::Erase => Err(io::Error::other("cannot be redone")),
        }
    }
}

/// The folders a report touched (to refresh views showing them).
pub fn touched_folders(report: &Report) -> HashSet<PathBuf> {
    report
        .created
        .iter()
        .chain(report.failed.iter().map(|f| &f.path))
        .filter_map(|p| p.parent().map(Path::to_path_buf))
        .collect()
}
