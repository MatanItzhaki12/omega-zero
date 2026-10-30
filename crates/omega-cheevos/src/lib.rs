//! RetroAchievements, through rcheevos, the site's own client library (MIT, in rcheevos/): one
//! client for the handheld, signed in with the player's account, that identifies the game put in
//! by hashing its ROM, reads its memory every frame and says when an achievement is earned.
//!
//! rcheevos asks for the network through this crate: each request goes out on a thread of its
//! own through `curl`, as label fetches do, and its answer is handed back to the client the next
//! time it is pumped, on whichever thread pumps it. Every call into the client is made under one
//! lock, so the main thread (signing in, between games) and the emulator's (a game's frames)
//! never meet inside it.

use std::ffi::{c_char, c_int, c_uint, c_void, CStr, CString};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use omega_retro::{CoreMemory, MemoryDescriptor};

/// The consoles omega-zero plays, by rcheevos' numbers.
pub const CONSOLE_SNES: u32 = 3;
pub const CONSOLE_GB: u32 = 4;
pub const CONSOLE_GBA: u32 = 5;
pub const CONSOLE_GBC: u32 = 6;

/// How long one request may take before it is given up as a network error the client retries.
const REQUEST_SECS: &str = "30";

/// rcheevos' answer for a request that could not reach the server and may be tried again.
const RETRYABLE: c_int = -2;

extern "C" {
    fn oc_start(hardcore: c_int) -> c_int;
    fn oc_hardcore() -> c_int;
    fn oc_user_agent(buffer: *mut c_char, size: usize) -> usize;
    fn oc_login_password(user: *const c_char, password: *const c_char);
    fn oc_login_token(user: *const c_char, token: *const c_char);
    fn oc_login_state(error: *mut *const c_char) -> c_int;
    fn oc_user_name() -> *const c_char;
    fn oc_user_token() -> *const c_char;
    fn oc_logout();
    #[allow(clippy::too_many_arguments)]
    fn oc_set_memory(
        descriptors: *const MemoryDescriptor,
        count: c_uint,
        console: u32,
        system: *mut u8,
        system_size: usize,
        save: *mut u8,
        save_size: usize,
        video: *mut u8,
        video_size: usize,
    );
    fn oc_load_game(console: u32, path: *const c_char);
    fn oc_game_state(error: *mut *const c_char) -> c_int;
    fn oc_unload_game();
    fn oc_summary(
        title: *mut *const c_char,
        id: *mut u32,
        unlocked: *mut u32,
        total: *mut u32,
        points_unlocked: *mut u32,
        points_total: *mut u32,
    ) -> c_int;
    fn oc_achievements(each: AchievementFn, user: *mut c_void);
    fn oc_frame();
    fn oc_idle();
    fn oc_reset();
    fn oc_answer(id: u32, status: c_int, body: *const c_char, length: usize);
}

type AchievementFn = extern "C" fn(
    user: *mut c_void,
    title: *const c_char,
    description: *const c_char,
    points: u32,
    unlocked: c_int,
    progress: *const c_char,
);

/// The client is made, and every oc_ call may be made.
static STARTED: AtomicBool = AtomicBool::new(false);
/// No request leaves the handheld: each is answered as a network error. For tests, which must
/// never send an account anywhere.
static OFFLINE: AtomicBool = AtomicBool::new(false);
/// Held across every call into the client.
static CLIENT: Mutex<()> = Mutex::new(());
/// Answers come back from the network, waiting to be handed to the client.
static ANSWERS: Mutex<Vec<(u32, c_int, Vec<u8>)>> = Mutex::new(Vec::new());
static EVENTS: Mutex<Vec<Event>> = Mutex::new(Vec::new());
/// What every request says it is, as RetroAchievements asks: omega-zero and rcheevos' versions.
static AGENT: Mutex<String> = Mutex::new(String::new());

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Runs `f` on the client, once it is made, under its lock.
fn with_client<R>(f: impl FnOnce() -> R) -> Option<R> {
    let _held = lock(&CLIENT);
    STARTED.load(Ordering::Acquire).then(f)
}

fn text(p: *const c_char) -> String {
    match p.is_null() {
        true => String::new(),
        // SAFETY: rcheevos hands NUL-terminated strings that live through the call
        false => unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned(),
    }
}

fn c(s: &str) -> CString {
    CString::new(s.replace('\0', "")).unwrap_or_default()
}

/// What happened in a game, for the player to be told.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventKind {
    Unlocked,
    LeaderboardStarted,
    LeaderboardFailed,
    LeaderboardSubmitted,
    /// Every achievement of the game is earned.
    Mastered,
    ServerError,
    /// An unlock could not reach the server; it is kept and sent again.
    Disconnected,
    Reconnected,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    pub kind: EventKind,
    pub title: String,
    pub description: String,
    pub points: u32,
}

/// Where signing in stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Login {
    Out,
    Pending,
    /// Signed in, with the name the site shows and the token to sign in with from now on.
    In {
        name: String,
        token: String,
    },
    Refused(String),
}

/// Where loading the game in the handheld's achievements stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GameLoad {
    None,
    Pending,
    Loaded,
    /// No achievements for it, or the server would not say.
    Refused(String),
}

/// The game's achievements, and how many of them the player has.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Summary {
    /// The game's name on RetroAchievements, and its number there: what the ROM was taken for.
    pub title: String,
    pub id: u32,
    pub unlocked: u32,
    pub total: u32,
    pub points_unlocked: u32,
    pub points_total: u32,
}

/// One of the game's achievements, as the player stands with it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Achievement {
    pub title: String,
    pub description: String,
    pub points: u32,
    pub unlocked: bool,
    /// How far along a measured one is, as "3/10"; empty for the rest.
    pub progress: String,
}

/// Makes the client, in hardcore or not; `version` is omega-zero's, for the requests' agent.
/// Calling it again only switches hardcore.
pub fn start(hardcore: bool, version: &str) -> bool {
    let _held = lock(&CLIENT);
    // SAFETY: under the lock, as every call into the client
    let made = unsafe { oc_start(c_int::from(hardcore)) } != 0;
    if made {
        let mut clause = vec![0u8; 128];
        // SAFETY: the buffer is as long as said
        let n = unsafe { oc_user_agent(clause.as_mut_ptr() as *mut c_char, clause.len()) };
        clause.truncate(n.min(clause.len()));
        *lock(&AGENT) = format!(
            "omega-zero/{version} {}",
            String::from_utf8_lossy(&clause).trim_end_matches('\0')
        );
        STARTED.store(true, Ordering::Release);
    }
    made
}

/// Keeps every request on the handheld from now on, answered as a network error the client
/// retries: for tests, so none ever reaches RetroAchievements.
pub fn go_offline() {
    OFFLINE.store(true, Ordering::Release);
}

pub fn login_with_password(user: &str, password: &str) {
    let (user, password) = (c(user), c(password));
    // SAFETY: the strings outlive the call, which copies what it keeps
    with_client(|| unsafe { oc_login_password(user.as_ptr(), password.as_ptr()) });
}

pub fn login_with_token(user: &str, token: &str) {
    let (user, token) = (c(user), c(token));
    // SAFETY: as above
    with_client(|| unsafe { oc_login_token(user.as_ptr(), token.as_ptr()) });
}

pub fn logout() {
    // SAFETY: under the client's lock
    with_client(|| unsafe { oc_logout() });
}

pub fn login() -> Login {
    with_client(|| {
        let mut error = std::ptr::null();
        // SAFETY: under the lock; the error lives in the shim
        match unsafe { oc_login_state(&mut error) } {
            1 => Login::Pending,
            2 => Login::In {
                name: text(unsafe { oc_user_name() }),
                token: text(unsafe { oc_user_token() }),
            },
            3 => Login::Refused(text(error)),
            _ => Login::Out,
        }
    })
    .unwrap_or(Login::Out)
}

/// Starts the achievements of the game `rom` of `console` the core has just loaded, read from
/// `memory`. Called on the thread that runs the core, which `frame` must be called on too.
pub fn begin_game(console: u32, rom: &Path, memory: &CoreMemory) {
    let path = c(&rom.to_string_lossy());
    with_client(|| unsafe {
        // SAFETY: the shim keeps the region pointers, the core's, for as long as the game is
        // loaded; `end_game` drops them before the core goes
        oc_set_memory(
            memory.map.as_ptr(),
            memory.map.len() as c_uint,
            console,
            memory.system.0,
            memory.system.1,
            memory.save.0,
            memory.save.1,
            memory.video.0,
            memory.video.1,
        );
        oc_load_game(console, path.as_ptr());
    });
}

pub fn game_load() -> GameLoad {
    with_client(|| {
        let mut error = std::ptr::null();
        // SAFETY: under the lock
        match unsafe { oc_game_state(&mut error) } {
            1 => GameLoad::Pending,
            2 => GameLoad::Loaded,
            3 => GameLoad::Refused(text(error)),
            _ => GameLoad::None,
        }
    })
    .unwrap_or(GameLoad::None)
}

/// The game's achievements, once they are loaded.
pub fn summary() -> Option<Summary> {
    with_client(|| {
        let mut title = std::ptr::null();
        let (mut id, mut unlocked, mut total, mut points_unlocked, mut points_total) =
            (0, 0, 0, 0, 0);
        // SAFETY: under the lock; the title lives in the client with the game
        let loaded = unsafe {
            oc_summary(
                &mut title,
                &mut id,
                &mut unlocked,
                &mut total,
                &mut points_unlocked,
                &mut points_total,
            )
        } != 0;
        loaded.then(|| Summary {
            title: text(title),
            id,
            unlocked,
            total,
            points_unlocked,
            points_total,
        })
    })
    .flatten()
}

/// The game's achievements, once they are loaded: none before, or with no game.
pub fn achievements() -> Vec<Achievement> {
    extern "C" fn each(
        user: *mut c_void,
        title: *const c_char,
        description: *const c_char,
        points: u32,
        unlocked: c_int,
        progress: *const c_char,
    ) {
        // SAFETY: `user` is the Vec handed to oc_achievements below, alive through its call
        let list = unsafe { &mut *(user as *mut Vec<Achievement>) };
        list.push(Achievement {
            title: text(title),
            description: text(description),
            points,
            unlocked: unlocked != 0,
            progress: text(progress),
        });
    }
    let mut list: Vec<Achievement> = Vec::new();
    // SAFETY: under the lock; `each` only pushes onto `list`, which outlives the call
    with_client(|| unsafe { oc_achievements(each, &mut list as *mut _ as *mut c_void) });
    list
}

/// The game is played in hardcore: no states loaded, no rewinding.
pub fn hardcore_active() -> bool {
    // SAFETY: under the lock
    with_client(|| unsafe { oc_hardcore() } != 0).unwrap_or(false)
}

/// One frame of the game has run: hands the client the answers come back, and lets it check
/// the game's memory for achievements.
pub fn frame() {
    with_client(|| {
        deliver();
        // SAFETY: under the lock, on the thread that runs the core
        unsafe { oc_frame() };
    });
}

/// Hands the client the answers come back and lets it do what it waits to do, without reading
/// the game's memory: between games, or with the game paused.
pub fn pump() {
    with_client(|| {
        deliver();
        // SAFETY: under the lock
        unsafe { oc_idle() };
    });
}

/// The game's memory jumped, as a state loads or the game rewinds: achievements start watching
/// again from here, so none is earned by the jump.
pub fn reset() {
    // SAFETY: under the lock
    with_client(|| unsafe { oc_reset() });
}

/// The game is put away: the client lets go of it and of its memory, before the core goes.
pub fn end_game() {
    // SAFETY: under the lock
    with_client(|| unsafe { oc_unload_game() });
}

pub fn take_events() -> Vec<Event> {
    std::mem::take(&mut *lock(&EVENTS))
}

/// Hands the answers come back to the client. Called under its lock.
fn deliver() {
    let answers = std::mem::take(&mut *lock(&ANSWERS));
    for (id, status, mut body) in answers {
        let length = body.len();
        body.push(0);
        // SAFETY: under the lock; the body is NUL-terminated and lives through the call
        unsafe { oc_answer(id, status, body.as_ptr() as *const c_char, length) };
    }
}

/// The client asks for the network: the request goes out on a thread of its own, and its
/// answer waits in ANSWERS for the next pump. Called by the shim, under the client's lock.
#[no_mangle]
pub extern "C" fn omega_cheevos_server_call(
    id: u32,
    url: *const c_char,
    post_data: *const c_char,
    content_type: *const c_char,
) {
    let url = text(url);
    let post = (!post_data.is_null()).then(|| text(post_data));
    let content_type = text(content_type);
    let agent = lock(&AGENT).clone();
    if OFFLINE.load(Ordering::Acquire) {
        lock(&ANSWERS).push((id, RETRYABLE, Vec::new()));
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("omega-cheevos-net".into())
        .spawn(move || {
            let (status, body) = fetch(&url, post.as_deref(), &content_type, &agent);
            lock(&ANSWERS).push((id, status, body));
        });
    if spawned.is_err() {
        lock(&ANSWERS).push((id, RETRYABLE, Vec::new()));
    }
}

#[no_mangle]
pub extern "C" fn omega_cheevos_event(
    kind: u32,
    title: *const c_char,
    description: *const c_char,
    points: u32,
) {
    let kind = match kind {
        1 => EventKind::Unlocked,
        2 => EventKind::LeaderboardStarted,
        3 => EventKind::LeaderboardFailed,
        4 => EventKind::LeaderboardSubmitted,
        15 => EventKind::Mastered,
        16 => EventKind::ServerError,
        17 => EventKind::Disconnected,
        18 => EventKind::Reconnected,
        _ => return,
    };
    lock(&EVENTS).push(Event {
        kind,
        title: text(title),
        description: text(description),
        points,
    });
}

/// One request through curl: the post data goes in on its standard input, so a password never
/// stands on a command line, and the HTTP status comes after the body on a line of its own.
fn fetch(url: &str, post: Option<&str>, content_type: &str, agent: &str) -> (c_int, Vec<u8>) {
    let mut cmd = Command::new("curl");
    cmd.args(["-sS", "--max-time", REQUEST_SECS, "-A", agent])
        .args(["-w", "\n%{http_code}"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if post.is_some() {
        cmd.args(["--data-binary", "@-"]);
        if !content_type.is_empty() {
            cmd.args(["-H", &format!("Content-Type: {content_type}")]);
        }
        cmd.stdin(Stdio::piped());
    } else {
        cmd.stdin(Stdio::null());
    }
    cmd.arg(url);
    let Ok(mut child) = cmd.spawn() else {
        return (RETRYABLE, Vec::new());
    };
    if let (Some(post), Some(mut stdin)) = (post, child.stdin.take()) {
        use std::io::Write;
        let _ = stdin.write_all(post.as_bytes());
    }
    match child.wait_with_output() {
        Ok(out) => answer_of(&out.stdout),
        Err(_) => (RETRYABLE, Vec::new()),
    }
}

/// curl's output split into the body and the status on its last line; a status curl could not
/// give, as when the network is down, is one the client retries.
pub fn answer_of(out: &[u8]) -> (c_int, Vec<u8>) {
    let at = out.iter().rposition(|b| *b == b'\n');
    let (body, code) = match at {
        Some(i) => (&out[..i], &out[i + 1..]),
        None => (&out[..0], out),
    };
    let status = std::str::from_utf8(code)
        .ok()
        .and_then(|s| s.trim().parse::<c_int>().ok())
        .filter(|s| *s > 0)
        .unwrap_or(RETRYABLE);
    (status, body.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_status_is_the_line_after_the_body() {
        assert_eq!(
            answer_of(b"{\"Success\":true}\n200"),
            (200, b"{\"Success\":true}".to_vec())
        );
        assert_eq!(
            answer_of(b"line one\nline two\n404"),
            (404, b"line one\nline two".to_vec())
        );
    }

    #[test]
    fn no_status_is_a_network_error_the_client_retries() {
        assert_eq!(answer_of(b"").0, RETRYABLE);
        assert_eq!(answer_of(b"\n000").0, RETRYABLE);
    }

    #[test]
    fn nothing_happens_before_the_client_is_made() {
        assert_eq!(login(), Login::Out);
        assert_eq!(game_load(), GameLoad::None);
        assert!(!hardcore_active());
        frame();
        pump();
    }

    #[test]
    fn the_client_is_made_and_says_what_it_is() {
        assert!(start(false, "9.9.9"));
        let agent = lock(&AGENT).clone();
        assert!(agent.starts_with("omega-zero/9.9.9 rcheevos/"), "{agent}");
        assert_eq!(login(), Login::Out);
        assert_eq!(summary(), None);
    }
}
