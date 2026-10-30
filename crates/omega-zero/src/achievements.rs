//! RetroAchievements: with Settings > Gameplay > RetroAchievements on, omega-zero signs in to
//! the player's account and every Game Boy, Game Boy Color, GBA and SNES game is checked for
//! achievements as it plays, each one earned said in a toast.
//!
//! The account is kept in Config/retroachievements.ini, which the player writes once:
//!
//! ```text
//! username = MyName
//! password = MyPassword
//! ```
//!
//! On the first sign-in the password is swapped for the token the site hands back, as every
//! RetroAchievements emulator keeps it, and the file then holds only the name and the token.
//!
//! Hardcore, on the same panel, plays as the site's hardcore asks: no state is loaded, no game
//! rewinds and none picks up from where it was put away; unlocks count as hardcore.

use std::path::Path;

use omega_store::{atomic_write, Platform, CONFIG_DIR};
use omega_ui::Toast;

use crate::app::App;

pub const ACCOUNT_FILE: &str = "retroachievements.ini";

/// The longest a title is shown in a toast before it is cut short.
const TOAST_CHARS: usize = 30;

/// The player's RetroAchievements account, as Config/retroachievements.ini holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    pub user: String,
    pub password: Option<String>,
    pub token: Option<String>,
}

impl Account {
    pub fn load(root: &Path) -> Option<Account> {
        let fields = omega_store::ini::read(root, &format!("{CONFIG_DIR}/{ACCOUNT_FILE}"));
        let field = |k: &str| fields.get(k).filter(|v| !v.is_empty()).cloned();
        Some(Account {
            user: field("username")?,
            password: field("password"),
            token: field("token"),
        })
    }

    /// Keeps the name and the token the site handed back, and drops the password.
    pub fn keep_token(root: &Path, user: &str, token: &str) -> std::io::Result<()> {
        let text = format!(
            "# RetroAchievements account. omega-zero swapped the password for this token on\n\
             # its first sign-in; to sign in as someone else, put username and password back.\n\
             username = {user}\ntoken = {token}\n"
        );
        atomic_write(&root.join(CONFIG_DIR).join(ACCOUNT_FILE), text.as_bytes())
    }
}

/// The console a system's games are checked as; none for one RetroAchievements does not read
/// here, the PlayStation, whose discs are not hashed.
pub fn console_of(platform: Platform) -> Option<u32> {
    match platform {
        Platform::Gb => Some(omega_cheevos::CONSOLE_GB),
        Platform::Gbc => Some(omega_cheevos::CONSOLE_GBC),
        Platform::Gba => Some(omega_cheevos::CONSOLE_GBA),
        Platform::Snes => Some(omega_cheevos::CONSOLE_SNES),
        Platform::Ps1 => None,
    }
}

/// `text` cut to what a toast holds, kept for the toast's life: the few toasts a session shows.
fn shown(text: &str) -> &'static str {
    let cut: String = match text.chars().count() > TOAST_CHARS {
        true => {
            let mut t: String = text.chars().take(TOAST_CHARS - 1).collect();
            t.push('…');
            t
        }
        false => text.to_string(),
    };
    Box::leak(cut.into_boxed_str())
}

/// Where the session with RetroAchievements stands, as the app follows it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cheevos {
    /// Signing in was asked for, and its answer not yet told.
    pub signing_in: bool,
    /// The name signed in as.
    pub signed_in: Option<String>,
    /// Why the last sign-in was refused.
    pub refused: Option<String>,
    /// The game whose summary is still to be told when its achievements load.
    pub told_game: bool,
}

impl App {
    /// Whether games are checked for achievements: switched on, with an account to sign in.
    pub fn achievements_on(&self) -> bool {
        self.state.cheevos
    }

    /// Hardcore is on, for a game played with achievements: no state loads and no rewinding.
    pub fn hardcore(&self) -> bool {
        self.state.cheevos && self.state.cheevos_hardcore && self.cheevos.signed_in.is_some()
    }

    /// The console the game in the handheld is checked as, when achievements are on.
    pub fn achievements_console(&self) -> u32 {
        match self.state.cheevos {
            true => console_of(self.platform).unwrap_or(0),
            false => 0,
        }
    }

    /// Starts the client and signs in, as the boot does and as switching achievements on does.
    pub(crate) fn start_achievements(&mut self) {
        if !self.state.cheevos {
            return;
        }
        let Some(root) = self.root.clone() else {
            return;
        };
        omega_cheevos::start(self.state.cheevos_hardcore, env!("CARGO_PKG_VERSION"));
        let Some(account) = Account::load(&root) else {
            self.cheevos.refused = Some(format!("No account in Config/{ACCOUNT_FILE}"));
            return;
        };
        match (&account.token, &account.password) {
            (Some(token), _) => omega_cheevos::login_with_token(&account.user, token),
            (None, Some(password)) => omega_cheevos::login_with_password(&account.user, password),
            (None, None) => {
                self.cheevos.refused = Some("No password or token in the account".into());
                return;
            }
        }
        self.cheevos.signing_in = true;
        self.cheevos.refused = None;
    }

    /// Switching achievements off signs out; a game already started keeps its own until it is
    /// put away.
    pub(crate) fn stop_achievements(&mut self) {
        omega_cheevos::logout();
        self.cheevos = Cheevos::default();
    }

    /// What the RetroAchievements row's note says: who is signed in, or what is missing.
    pub fn achievements_note(&self) -> String {
        if !self.state.cheevos {
            return format!("Sign in with Config/{ACCOUNT_FILE}");
        }
        match (&self.cheevos.signed_in, &self.cheevos.refused) {
            (Some(name), _) => format!("Signed in as {name}"),
            (None, Some(why)) => why.clone(),
            (None, None) if self.cheevos.signing_in => "Signing in…".into(),
            (None, None) => format!("Sign in with Config/{ACCOUNT_FILE}"),
        }
    }

    /// Follows the client: hands it the network's answers, and tells the player what happened.
    pub(crate) fn tick_achievements(&mut self) {
        if !self.state.cheevos {
            return;
        }
        omega_cheevos::pump();
        let now = self.now();
        if self.cheevos.signing_in {
            match omega_cheevos::login() {
                omega_cheevos::Login::In { name, token } => {
                    self.cheevos.signing_in = false;
                    self.cheevos.signed_in = Some(name.clone());
                    if let (Some(root), Some(account)) =
                        (&self.root, self.root.as_deref().and_then(Account::load))
                    {
                        // the password is swapped for the token, once
                        if account.password.is_some() && !token.is_empty() {
                            if let Err(e) = Account::keep_token(root, &account.user, &token) {
                                eprintln!("omega-zero: {ACCOUNT_FILE}: {e}");
                            }
                        }
                    }
                    self.hud
                        .toast(Toast::Cheevos(shown(&format!("Signed in as {name}"))), now);
                }
                omega_cheevos::Login::Refused(why) => {
                    self.cheevos.signing_in = false;
                    self.cheevos.refused = Some(why.clone());
                    eprintln!("omega-zero: cheevos: sign-in refused: {why}");
                    self.hud.toast(Toast::Cheevos("Sign-in failed"), now);
                }
                _ => {}
            }
        }
        if !self.cheevos.told_game {
            if let Some(s) = omega_cheevos::summary() {
                self.cheevos.told_game = true;
                if s.total > 0 {
                    let line = format!("{} of {} unlocked", s.unlocked, s.total);
                    self.hud.toast(Toast::Cheevos(shown(&line)), now);
                }
            }
        }
        for event in omega_cheevos::take_events() {
            use omega_cheevos::EventKind as E;
            let toast = match event.kind {
                E::Unlocked => Toast::Achievement(shown(&event.title)),
                E::Mastered => Toast::Cheevos(shown(&format!("Mastered {}", event.title))),
                E::LeaderboardSubmitted => {
                    Toast::Cheevos(shown(&format!("{}: {}", event.title, event.description)))
                }
                E::Disconnected => Toast::Cheevos("Unlocks waiting for network"),
                E::Reconnected => Toast::Cheevos("Unlocks sent"),
                E::ServerError => {
                    eprintln!(
                        "omega-zero: cheevos: {}: {}",
                        event.title, event.description
                    );
                    continue;
                }
                E::LeaderboardStarted | E::LeaderboardFailed => continue,
            };
            self.hud.toast(toast, now);
        }
    }

    /// A game goes in: its summary is told once its achievements load.
    pub(crate) fn achievements_game_starts(&mut self) {
        self.cheevos.told_game = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_account_takes_a_password_once_and_keeps_the_token() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join(CONFIG_DIR)).unwrap();
        let file = d.path().join(CONFIG_DIR).join(ACCOUNT_FILE);
        assert_eq!(Account::load(d.path()), None);
        std::fs::write(&file, "username = Ash\r\npassword = pikachu\r\n").unwrap();
        assert_eq!(
            Account::load(d.path()),
            Some(Account {
                user: "Ash".into(),
                password: Some("pikachu".into()),
                token: None
            })
        );
        Account::keep_token(d.path(), "Ash", "t0k3n").unwrap();
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(!text.contains("pikachu"), "the password was kept: {text}");
        assert_eq!(
            Account::load(d.path()),
            Some(Account {
                user: "Ash".into(),
                password: None,
                token: Some("t0k3n".into())
            })
        );
    }

    #[test]
    fn the_handhelds_and_the_snes_have_achievements_and_the_playstation_none_here() {
        assert_eq!(console_of(Platform::Gba), Some(5));
        assert_eq!(console_of(Platform::Gb), Some(4));
        assert_eq!(console_of(Platform::Gbc), Some(6));
        assert_eq!(console_of(Platform::Snes), Some(3));
        assert_eq!(console_of(Platform::Ps1), None);
    }

    #[test]
    fn a_long_title_is_cut_for_its_toast() {
        let t = shown("The Legend of Zelda: A Link to the Past and Four Swords");
        assert_eq!(t.chars().count(), TOAST_CHARS);
        assert!(t.ends_with('…'));
        assert_eq!(shown("Short"), "Short");
    }
}
