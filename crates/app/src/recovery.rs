//! AutoRecover (UI-18): every window keeps the engine's autosave (FI-03) in its own folder under the recovery
//! root, guarded by a lock file it holds while it runs. A copy is deleted once the publication is saved or
//! discarded; a copy whose window is gone (a crash, a power cut) is offered back at the next start.

use crate::{NewpubApp, files, widgets};
use newpub_engine::SessionAction;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Name of the file holding the path of the publication a recovery folder belongs to.
const ORIGIN: &str = "origin.txt";

pub(crate) struct Recovery {
    root: PathBuf,
    /// This window's folder.
    dir: PathBuf,
    /// Held while the window runs; other windows see the folder is in use.
    _lock: File,
    /// The publication path last written to `origin.txt`.
    origin: Option<PathBuf>,
    was_dirty: bool,
    /// Copies left behind by windows that are gone.
    pub(crate) offers: Vec<Offer>,
    /// Show the start screen once the offers are declined.
    pub(crate) picker_after: bool,
}

#[derive(Clone)]
pub(crate) struct Offer {
    dir: PathBuf,
    title: String,
    origin: Option<PathBuf>,
    when: SystemTime,
}

fn lock_path(root: &Path, dir_name: &str) -> PathBuf {
    root.join(format!("{dir_name}.lock"))
}

fn autosave_files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|r| r.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "newspub")).collect())
        .unwrap_or_default();
    v.sort();
    v
}

/// Folders under `root` whose window is gone: their lock file is missing or can be locked.
fn orphans(root: &Path, own: &Path) -> Vec<Offer> {
    let Ok(entries) = std::fs::read_dir(root) else { return vec![] };
    let mut out = vec![];
    for e in entries.flatten() {
        let dir = e.path();
        let Some(name) = dir.file_name().and_then(|n| n.to_str()).map(str::to_string) else { continue };
        if !dir.is_dir() || dir == own {
            continue;
        }
        let free =
            match File::options().read(true).write(true).create(true).truncate(false).open(lock_path(root, &name)) {
                Ok(f) => f.try_lock().is_ok(),
                Err(_) => false,
            };
        if !free {
            continue;
        }
        let saves = autosave_files(&dir);
        let Some(newest) = saves.last() else {
            let _ = std::fs::remove_dir_all(&dir);
            let _ = std::fs::remove_file(lock_path(root, &name));
            continue;
        };
        let origin = std::fs::read_to_string(dir.join(ORIGIN))
            .ok()
            .map(|s| PathBuf::from(s.trim()))
            .filter(|p| !p.as_os_str().is_empty());
        let title = origin
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Untitled publication".into());
        let when = std::fs::metadata(newest).and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
        out.push(Offer { dir, title, origin, when });
    }
    out.sort_by_key(|o| std::cmp::Reverse(o.when));
    out
}

fn forget(root: &Path, dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
    if let Some(name) = dir.file_name().and_then(|n| n.to_str()) {
        let _ = std::fs::remove_file(lock_path(root, name));
    }
}

impl Drop for Recovery {
    /// A window that closes with nothing to recover leaves nothing behind.
    fn drop(&mut self) {
        if autosave_files(&self.dir).is_empty() {
            forget(&self.root, &self.dir);
        }
    }
}

fn ago(t: SystemTime) -> String {
    let s = SystemTime::now().duration_since(t).map(|d| d.as_secs()).unwrap_or(0);
    match s {
        0..60 => "moments ago".into(),
        60..3600 => format!("{} min ago", s / 60),
        3600..86400 => format!("{} h ago", s / 3600),
        _ => format!("{} days ago", s / 86400),
    }
}

impl NewpubApp {
    /// Keeps AutoRecover copies under `root`, written every `every_actions` changes.
    pub fn with_recovery(mut self, root: PathBuf, every_actions: u32) -> NewpubApp {
        let stamp = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
        let name = format!("{}-{stamp}", std::process::id());
        let dir = root.join(&name);
        let lock = std::fs::create_dir_all(&dir).and_then(|_| {
            File::options().read(true).write(true).create(true).truncate(false).open(lock_path(&root, &name))
        });
        let Ok(lock) = lock else {
            eprintln!("newpub: AutoRecover is off: cannot write to {}", root.display());
            return self;
        };
        if lock.try_lock().is_err() {
            return self;
        }
        let action =
            SessionAction::SetAutosave { dir: dir.to_string_lossy().to_string(), every_actions: every_actions.max(1) };
        if self.act(action).is_none() {
            return self;
        }
        let offers = orphans(&root, &dir);
        self.recovery =
            Some(Recovery { root, dir, _lock: lock, origin: None, was_dirty: false, offers, picker_after: false });
        self
    }

    /// Every frame: records which file the copy belongs to, and drops the copy once the publication is clean.
    pub(crate) fn recovery_tick(&mut self) {
        let path = self.session.path.clone();
        let dirty = self.session.dirty;
        let Some(r) = self.recovery.as_mut() else { return };
        if r.origin != path {
            let text = path.as_ref().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
            let _ = std::fs::write(r.dir.join(ORIGIN), text);
            r.origin = path;
        }
        if r.was_dirty && !dirty {
            Self::clear_copies(r);
        }
        r.was_dirty = dirty;
    }

    fn clear_copies(r: &Recovery) {
        for f in autosave_files(&r.dir) {
            let _ = std::fs::remove_file(f);
        }
    }

    /// The user discarded the unsaved publication on purpose (Don't Save): its copy goes too.
    pub(crate) fn recovery_discard(&mut self) {
        if let Some(r) = self.recovery.as_ref() {
            Self::clear_copies(r);
        }
    }

    /// "Recover unsaved work", shown at startup when a window left a copy behind.
    pub(crate) fn recovery_prompt(&mut self, ctx: &egui::Context) {
        let Some(offers) = self.recovery.as_ref().map(|r| r.offers.clone()).filter(|o| !o.is_empty()) else { return };
        let mut recover = None;
        let (mut discard_all, mut later) = (false, false);
        files::dialog_window("Recover unsaved work").show(ctx, |ui| {
            ui.set_max_width(420.0);
            widgets::hint(ui, "newpub closed before these publications were saved. Recover one to keep working on it.");
            ui.add_space(6.0);
            for (i, o) in offers.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&o.title).font(crate::theme::semibold(13.0)));
                    ui.label(egui::RichText::new(ago(o.when)).color(widgets::pal(ui).text_muted));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::primary_button(ui, &format!("Recover {}", o.title)).clicked() {
                            recover = Some(i);
                        }
                    });
                });
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if widgets::secondary_button(ui, "Discard All").clicked() {
                    discard_all = true;
                }
                if widgets::secondary_button(ui, "Later").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    later = true;
                }
            });
        });
        let Some(r) = self.recovery.as_mut() else { return };
        if later {
            r.offers.clear();
        }
        if discard_all {
            for o in std::mem::take(&mut r.offers) {
                forget(&r.root, &o.dir);
            }
        }
        if (later || discard_all) && std::mem::take(&mut r.picker_after) {
            self.open_picker();
            return;
        }
        if let Some(i) = recover {
            let o = r.offers.remove(i);
            r.picker_after = false;
            let root = r.root.clone();
            let own = r.dir.clone();
            if self.act(SessionAction::RecoverAutosave { dir: o.dir.to_string_lossy().to_string() }).is_some() {
                // The copy moves into this window's folder, so it stays safe until the publication is saved.
                if let Some(newest) = autosave_files(&o.dir).pop()
                    && let Some(name) = newest.file_name()
                {
                    let _ = std::fs::rename(&newest, own.join(name));
                }
                forget(&root, &o.dir);
                self.session.path = o.origin.clone();
                if let Some(r) = self.recovery.as_mut() {
                    r.was_dirty = true;
                }
                self.page = 0;
                self.selection.clear();
                self.end_text_edit();
                self.view.reset_transient();
                self.reset_gesture();
                self.thumbs.clear();
                if matches!(self.dialog, crate::Dialog::Picker(_)) {
                    self.dialog = crate::Dialog::None;
                }
                self.status = format!("Recovered {}: save it to keep it", o.title);
            }
        }
    }
}
