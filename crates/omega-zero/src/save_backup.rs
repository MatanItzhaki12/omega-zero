//! Save Backup: every save on the card copied to a folder on a NAS, as it is saved.
//!
//! What is backed up is the card's Saves folder as it stands, whatever wrote it: omega-zero's
//! own `.sav` files, and whatever a core keeps there (a PlayStation's memory cards, a disc
//! game's disc), its folders and all. A system or emulator added later is backed up with no
//! change here, since nothing here knows one from another.
//!
//! A list on the card, `Config/save_backup.list`, holds what the NAS has: each file's path in
//! Saves, its size and its CRC-32, as last copied. A pass copies every file whose size or CRC
//! differs, the newest of it, over the one on the NAS; so a save made twice while the NAS was
//! away is copied once, as it stands. The first pass, or one after the NAS or the folder
//! changes, finds the list empty and copies everything.
//!
//! Passes run on a thread of their own: when a game is saved, when Wi-Fi comes up, every few
//! minutes while anything waits, and now and then besides for what a core wrote by itself. A
//! pass that cannot reach the NAS leaves the list as it was, and the next one tries again.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::nas::{Login, Nas, NasFail};

/// The list of what the NAS has, under the card's root.
const LIST: &str = "Config/save_backup.list";

/// How long after a pass that left saves waiting the next tries, and how long between passes
/// otherwise, for a save a core wrote with no word to omega-zero.
const RETRY: Duration = Duration::from_secs(60);
const LOOK_AGAIN: Duration = Duration::from_secs(300);

/// Where saves are copied to: the NAS, the share on it, and the folder in the share, its parts
/// apart by `/`, none for the share's own top.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Target {
    pub login: Login,
    pub share: String,
    pub dir: String,
}

impl Target {
    /// The target as the list names it, so a list kept for another is known not to be its.
    fn key(&self) -> String {
        format!(
            "{}|{}|{}",
            self.login.host.trim().to_lowercase(),
            self.share.to_lowercase(),
            self.dir.trim_matches('/').to_lowercase()
        )
    }

    /// Where the file at `rel` in Saves goes in the share.
    pub fn remote(&self, rel: &str) -> String {
        match self.dir.trim_matches('/') {
            "" => rel.to_string(),
            dir => format!("{dir}/{rel}"),
        }
    }
}

/// A file as the list keeps it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Kept {
    pub size: u64,
    pub crc: u32,
}

/// What the NAS has, by each file's path in Saves, and the target it is had for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct List {
    pub target: String,
    pub files: HashMap<String, Kept>,
}

impl List {
    pub fn read(root: &Path) -> List {
        let Ok(text) = std::fs::read_to_string(root.join(LIST)) else {
            return List::default();
        };
        let mut lines = text.lines();
        let target = lines
            .next()
            .and_then(|l| l.strip_prefix("target\t"))
            .unwrap_or_default()
            .to_string();
        let files = lines
            .filter_map(|line| {
                let mut f = line.split('\t');
                let (path, size, crc) = (f.next()?, f.next()?, f.next()?);
                Some((
                    path.to_string(),
                    Kept {
                        size: size.parse().ok()?,
                        crc: u32::from_str_radix(crc, 16).ok()?,
                    },
                ))
            })
            .collect();
        List { target, files }
    }

    pub fn write(&self, root: &Path) -> std::io::Result<()> {
        let mut paths: Vec<&String> = self.files.keys().collect();
        paths.sort();
        let mut text = format!("target\t{}\n", self.target);
        for path in paths {
            let k = self.files[path];
            text.push_str(&format!("{path}\t{}\t{:08x}\n", k.size, k.crc));
        }
        let at = root.join(LIST);
        if let Some(dir) = at.parent() {
            std::fs::create_dir_all(dir)?;
        }
        omega_store::atomic_write(&at, text.as_bytes())
    }
}

/// Every file in `saves`, as its path from there with `/` between its parts: hidden files, as a
/// write half done is, left out.
pub fn saved_files(saves: &Path) -> Vec<String> {
    fn walk(dir: &Path, rel: &str, out: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let path = match rel {
                "" => name.clone(),
                rel => format!("{rel}/{name}"),
            };
            match entry.file_type() {
                Ok(t) if t.is_dir() => walk(&entry.path(), &path, out),
                Ok(t) if t.is_file() => out.push(path),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(saves, "", &mut out);
    out.sort();
    out
}

/// The files in `saves` the NAS does not have as they stand, by `list`, with what they hold
/// now.
pub fn changed(saves: &Path, list: &List) -> Vec<(String, Vec<u8>)> {
    saved_files(saves)
        .into_iter()
        .filter_map(|rel| {
            let bytes = std::fs::read(saves.join(&rel)).ok()?;
            let now = Kept {
                size: bytes.len() as u64,
                crc: crate::label_sync::crc32(&bytes),
            };
            (list.files.get(&rel) != Some(&now)).then_some((rel, bytes))
        })
        .collect()
}

/// Where Save Backup stands, as Settings shows it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Status {
    /// A pass is copying now.
    pub busy: bool,
    /// How many saves the NAS does not have yet, as last counted.
    pub waiting: usize,
    /// When the last pass that left nothing waiting ended, in seconds since 1970.
    pub done_at: Option<u64>,
    /// Why the last pass stopped short, while saves still wait.
    pub fail: Option<NasFail>,
}

enum Ask {
    /// Look for saves to copy now.
    Pass,
    /// Copy to `target` from now on, or to nothing.
    Target(Option<Target>),
}

/// Save Backup's thread, which copies saves while it has a target.
pub struct SaveBackup {
    tx: Sender<Ask>,
    status: Arc<Mutex<Status>>,
}

impl SaveBackup {
    /// The thread for the card at `root`, copying to `target` while it is one.
    pub fn start(root: PathBuf, target: Option<Target>) -> SaveBackup {
        let (tx, rx) = channel();
        let status = Arc::new(Mutex::new(Status::default()));
        let shared = status.clone();
        let _ = std::thread::Builder::new()
            .name("omega-backup".into())
            .spawn(move || run(&root, target, &rx, &shared));
        SaveBackup { tx, status }
    }

    /// Looks for saves to copy now: after a save, or once the network is back.
    pub fn pass(&self) {
        let _ = self.tx.send(Ask::Pass);
    }

    /// Copies to `target` from now on; none stops.
    pub fn set_target(&self, target: Option<Target>) {
        let _ = self.tx.send(Ask::Target(target));
    }

    pub fn status(&self) -> Status {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn run(root: &Path, mut target: Option<Target>, rx: &Receiver<Ask>, status: &Mutex<Status>) {
    let mut wait = Duration::ZERO;
    loop {
        match rx.recv_timeout(wait) {
            Ok(Ask::Pass) | Err(RecvTimeoutError::Timeout) => {}
            Ok(Ask::Target(t)) => {
                target = t;
                if let Ok(mut s) = status.lock() {
                    *s = Status::default();
                }
            }
            Err(RecvTimeoutError::Disconnected) => return,
        }
        // asks that came together make one pass
        while let Ok(ask) = rx.try_recv() {
            if let Ask::Target(t) = ask {
                target = t;
            }
        }
        let Some(to) = &target else {
            wait = Duration::from_secs(3600);
            continue;
        };
        let left = pass(root, to, status);
        wait = if left > 0 { RETRY } else { LOOK_AGAIN };
    }
}

/// One pass: copies what the NAS does not have to `to`, keeping the list as each is copied.
/// Returns how many are left waiting.
pub fn pass(root: &Path, to: &Target, status: &Mutex<Status>) -> usize {
    let set = |f: &dyn Fn(&mut Status)| {
        if let Ok(mut s) = status.lock() {
            f(&mut s);
        }
    };
    let mut list = List::read(root);
    if list.target != to.key() {
        // another NAS or folder: it has nothing yet, and gets everything
        list = List {
            target: to.key(),
            files: HashMap::new(),
        };
    }
    let saves = crate::root::saves_dir(root);
    let todo = changed(&saves, &list);
    let total = todo.len();
    set(&|s| s.waiting = total);
    if todo.is_empty() {
        set(&|s| {
            s.fail = None;
            s.done_at.get_or_insert(now_secs());
        });
        return 0;
    }
    set(&|s| s.busy = true);
    let outcome = copy(root, to, &mut list, todo, status);
    let left = match &outcome {
        Ok(()) => 0,
        Err((left, _)) => *left,
    };
    set(&|s| {
        s.busy = false;
        s.waiting = left;
        match &outcome {
            Ok(()) => {
                s.fail = None;
                s.done_at = Some(now_secs());
            }
            Err((_, e)) => s.fail = Some(e.clone()),
        }
    });
    left
}

fn copy(
    root: &Path,
    to: &Target,
    list: &mut List,
    todo: Vec<(String, Vec<u8>)>,
    status: &Mutex<Status>,
) -> Result<(), (usize, NasFail)> {
    let total = todo.len();
    let mut nas = Nas::open(&to.login).map_err(|e| {
        eprintln!("omega-zero: save backup: {}: {e}", to.login.host);
        (total, e)
    })?;
    for (done, (rel, bytes)) in todo.into_iter().enumerate() {
        if let Err(e) = nas.put(&to.share, &to.remote(&rel), &bytes) {
            eprintln!("omega-zero: save backup: {rel}: {e}");
            let _ = list.write(root);
            return Err((total - done, e));
        }
        list.files.insert(
            rel,
            Kept {
                size: bytes.len() as u64,
                crc: crate::label_sync::crc32(&bytes),
            },
        );
        if let Ok(mut s) = status.lock() {
            s.waiting = total - done - 1;
        }
    }
    if let Err(e) = list.write(root) {
        eprintln!("omega-zero: save backup: {LIST}: {e}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target() -> Target {
        Target {
            login: Login {
                host: "192.168.1.5".into(),
                user: "ash".into(),
                password: "pikachu".into(),
            },
            share: "Backups".into(),
            dir: "RG SP/Saves".into(),
        }
    }

    #[test]
    fn a_save_goes_under_the_folder_picked() {
        assert_eq!(
            target().remote("GBA/Emerald.sav"),
            "RG SP/Saves/GBA/Emerald.sav"
        );
        let top = Target {
            dir: String::new(),
            ..target()
        };
        assert_eq!(top.remote("GBA/Emerald.sav"), "GBA/Emerald.sav");
    }

    #[test]
    fn every_file_in_saves_is_found_but_the_hidden_ones() {
        let d = tempfile::tempdir().unwrap();
        let saves = d.path();
        std::fs::create_dir_all(saves.join("GBA")).unwrap();
        std::fs::create_dir_all(saves.join("PS1/cards")).unwrap();
        std::fs::write(saves.join("GBA/Emerald.sav"), b"e").unwrap();
        std::fs::write(saves.join("GBA/.Emerald.sav.12.0.tmp"), b"half").unwrap();
        std::fs::write(saves.join("PS1/cards/Crash.mcd"), b"c").unwrap();
        assert_eq!(
            saved_files(saves),
            ["GBA/Emerald.sav", "PS1/cards/Crash.mcd"]
        );
    }

    #[test]
    fn only_what_changed_since_the_list_is_copied() {
        let d = tempfile::tempdir().unwrap();
        let saves = d.path();
        std::fs::create_dir_all(saves.join("GBA")).unwrap();
        std::fs::write(saves.join("GBA/Emerald.sav"), b"one").unwrap();
        std::fs::write(saves.join("GBA/Ruby.sav"), b"two").unwrap();
        let mut list = List::default();
        assert_eq!(
            changed(saves, &list).len(),
            2,
            "a first pass copies everything"
        );
        list.files.insert(
            "GBA/Emerald.sav".into(),
            Kept {
                size: 3,
                crc: crate::label_sync::crc32(b"one"),
            },
        );
        let todo: Vec<String> = changed(saves, &list).into_iter().map(|(p, _)| p).collect();
        assert_eq!(todo, ["GBA/Ruby.sav"]);
        // saved again while the NAS was away: the newest is what goes
        std::fs::write(saves.join("GBA/Emerald.sav"), b"three").unwrap();
        let todo = changed(saves, &list);
        assert!(todo
            .iter()
            .any(|(p, b)| p == "GBA/Emerald.sav" && b == b"three"));
    }

    #[test]
    fn the_list_reads_back_as_written() {
        let d = tempfile::tempdir().unwrap();
        let mut list = List {
            target: target().key(),
            files: HashMap::new(),
        };
        list.files.insert(
            "GBA/Emerald.sav".into(),
            Kept {
                size: 131072,
                crc: 0xdead_beef,
            },
        );
        list.write(d.path()).unwrap();
        assert_eq!(List::read(d.path()), list);
        assert_eq!(List::read(&d.path().join("none")), List::default());
    }

    #[test]
    fn a_nas_out_of_reach_leaves_every_save_waiting() {
        let d = tempfile::tempdir().unwrap();
        let saves = crate::root::saves_dir(d.path());
        std::fs::create_dir_all(saves.join("GBA")).unwrap();
        std::fs::write(saves.join("GBA/Emerald.sav"), b"e").unwrap();
        // nothing listens on port 9 here: refused at once
        let to = Target {
            login: Login {
                host: "127.0.0.1:9".into(),
                ..Login::default()
            },
            ..target()
        };
        let status = Mutex::new(Status::default());
        assert_eq!(pass(d.path(), &to, &status), 1);
        let s = status.lock().unwrap().clone();
        assert_eq!(s.waiting, 1);
        assert!(s.fail.is_some());
        assert!(!s.busy);
        assert!(
            List::read(d.path()).files.is_empty(),
            "the list took a save never copied"
        );
    }
}
