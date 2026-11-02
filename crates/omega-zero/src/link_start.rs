use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::time::{Duration, Instant};

use omega_retro::LinkChannel;

use crate::link_net::{Cancel, TcpLink, HOST_BOUND};
use crate::link_radio::{self, LinkRole, RadioFail};
use crate::netplay::{self, NetplayFail, NetplayLink, RelayHost};
use crate::netplay_lobby::{self, Announce, Announcer, Room};

#[cfg(feature = "device")]
pub const HOST_ADDR: &str = "10.42.0.1";

#[cfg(not(feature = "device"))]
pub const HOST_ADDR: &str = "127.0.0.1";

pub const DEFAULT_LINK_PORT: u16 = 7211;

const PORT_ENV: &str = "OMEGA_LINK_PORT";

pub fn link_port() -> u16 {
    let Some(raw) = std::env::var_os(PORT_ENV) else {
        return DEFAULT_LINK_PORT;
    };
    if raw.to_str().is_some_and(|s| s.trim().is_empty()) {
        return DEFAULT_LINK_PORT;
    }
    match raw.to_str().and_then(|s| s.trim().parse::<u16>().ok()) {
        None | Some(0) => {
            eprintln!(
                "omega-zero: {PORT_ENV}={:?} is not a port a link can meet on, using {DEFAULT_LINK_PORT}",
                raw.to_string_lossy()
            );
            DEFAULT_LINK_PORT
        }
        Some(port) => port,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkStep {
    Radio,
    Waiting,
    /// Online: a room being opened through a relay and put up in the lobby.
    Room,
    /// Online: the room being reached.
    Reaching,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkFail {
    Radio,
    NobodyCame,
    PeerVanished,
    Cancelled,
    /// Online: the lobby or the relay could not be reached.
    Lobby,
    /// Online: the room would not let this handheld in: full, or it has a password.
    Refused,
    /// Online: the room runs another emulator.
    OtherCore,
    /// Online: the handheld has joined no Wi-Fi network.
    NoWifi,
}

impl LinkStep {
    pub const ALL: [LinkStep; 4] = [
        LinkStep::Radio,
        LinkStep::Waiting,
        LinkStep::Room,
        LinkStep::Reaching,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn line(self) -> &'static str {
        match self {
            LinkStep::Radio => "Bringing the radio up",
            LinkStep::Waiting => "Looking for the other player",
            LinkStep::Room => "Opening a room online",
            LinkStep::Reaching => "Reaching the room",
        }
    }

    pub fn shown(self, warm: bool) -> LinkStep {
        match self {
            LinkStep::Radio if warm => LinkStep::Waiting,
            step => step,
        }
    }
}

impl LinkFail {
    pub const SHOWN: [LinkFail; 7] = [
        LinkFail::Radio,
        LinkFail::NobodyCame,
        LinkFail::PeerVanished,
        LinkFail::Lobby,
        LinkFail::Refused,
        LinkFail::OtherCore,
        LinkFail::NoWifi,
    ];

    pub fn shown(self) -> Option<usize> {
        LinkFail::SHOWN.iter().position(|f| *f == self)
    }

    pub fn line(self) -> &'static str {
        match self {
            LinkFail::Radio => "The radio did not come up",
            LinkFail::NobodyCame => "Nobody arrived",
            LinkFail::PeerVanished => "The other player vanished",
            LinkFail::Cancelled => "Cancelled",
            LinkFail::Lobby => "The online lobby did not answer",
            LinkFail::Refused => "The room would not let us in",
            LinkFail::OtherCore => "The room runs another emulator",
            LinkFail::NoWifi => "Join a Wi-Fi network first",
        }
    }
}

pub enum LinkProgress {
    At(LinkStep),
    Ready(Connected),
    Failed(LinkFail),
}

/// A link made: the connection the core's packets go over, the client the core plays as when
/// the other end numbered it (online), and the other player's name when they gave one.
pub struct Connected {
    pub link: Box<dyn LinkChannel>,
    pub client: Option<u16>,
    pub peer: Option<String>,
}

fn classify(e: &std::io::Error) -> LinkFail {
    match e.kind() {
        std::io::ErrorKind::Interrupted => LinkFail::Cancelled,
        std::io::ErrorKind::TimedOut => LinkFail::NobodyCame,
        _ => LinkFail::PeerVanished,
    }
}

type RadioUp = Box<dyn FnMut(LinkRole, &Cancel) -> Result<(), RadioFail> + Send>;
type RadioDown = Box<dyn FnMut() + Send>;
type Socket = Box<dyn FnMut(u16, &Cancel) -> std::io::Result<TcpLink> + Send>;

pub struct LinkStarter {
    rx: Receiver<LinkProgress>,
    cancel: Cancel,
    done: bool,
}

impl LinkStarter {
    pub fn spawn(role: LinkRole, port: u16) -> LinkStarter {
        LinkStarter::spawn_with(
            Box::new(link_radio::up),
            Box::new(link_radio::down),
            role,
            port,
            Box::new(move |port, cancel| match role {
                LinkRole::Host => TcpLink::host_until(HOST_ADDR, port, HOST_BOUND, cancel),
                LinkRole::Join => TcpLink::join_until(HOST_ADDR, port, HOST_BOUND, cancel),
            }),
        )
    }

    pub fn spawn_with(
        mut radio_up: RadioUp,
        mut radio_down: RadioDown,
        role: LinkRole,
        port: u16,
        mut socket: Socket,
    ) -> LinkStarter {
        let (tx, rx) = channel();
        let cancel = Cancel::new();
        let flag = cancel.clone();
        std::thread::spawn(move || {
            let _ = tx.send(LinkProgress::At(LinkStep::Radio));
            if let Err(e) = radio_up(role, &flag) {
                let fail = match &e {
                    RadioFail::NoHost => LinkFail::NobodyCame,
                    RadioFail::Cancelled => LinkFail::Cancelled,
                    RadioFail::Radio(why) => {
                        eprintln!("omega-zero: link: {role:?} could not bring the radio up: {why}");
                        LinkFail::Radio
                    }
                };
                radio_down();
                let _ = tx.send(LinkProgress::Failed(fail));
                return;
            }
            let _ = tx.send(LinkProgress::At(LinkStep::Waiting));
            eprintln!("omega-zero: link: {role:?} using {HOST_ADDR}:{port}");
            match socket(port, &flag) {
                Ok(link) => {
                    eprintln!("omega-zero: link: {role:?} connected on {HOST_ADDR}:{port}");
                    let ready = Connected {
                        link: Box::new(link),
                        client: None,
                        peer: None,
                    };
                    if tx.send(LinkProgress::Ready(ready)).is_err() {
                        eprintln!("omega-zero: link: nobody left to hand it to, radio back down");
                        radio_down();
                    }
                }
                Err(e) => {
                    eprintln!(
                        "omega-zero: link: {role:?} failed on {HOST_ADDR}:{port}: {e} (kind {:?})",
                        e.kind()
                    );
                    radio_down();
                    let _ = tx.send(LinkProgress::Failed(classify(&e)));
                }
            }
        });
        LinkStarter {
            rx,
            cancel,
            done: false,
        }
    }

    pub fn poll(&mut self) -> Option<LinkProgress> {
        if self.done {
            return None;
        }
        match self.rx.try_recv() {
            Ok(progress) => {
                self.done = matches!(progress, LinkProgress::Ready(_) | LinkProgress::Failed(_));
                Some(progress)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.done = true;
                Some(LinkProgress::Failed(LinkFail::PeerVanished))
            }
        }
    }

    pub fn cancel(&mut self) {
        self.cancel.cancel();
    }
}

impl Drop for LinkStarter {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

/// How long a room stays open for a player to come, and how long a room or a player is given
/// to say who they are.
pub const ROOM_OPEN: Duration = Duration::from_secs(300);
const ANSWER: Duration = Duration::from_secs(15);

/// What an online link is to do.
#[derive(Clone, Debug)]
pub enum OnlineJob {
    /// Open a room through the relay called `relay` and wait for a player.
    Host {
        nick: String,
        relay: String,
        game: String,
        crc: u32,
        /// The ROM the room's checksum is read off, on the link's thread: `crc` when there is
        /// none.
        rom: Option<std::path::PathBuf>,
    },
    /// Join a room from the lobby.
    Join { nick: String, room: Room },
    /// Join the room at an address, as RetroArch on the same network hosts one.
    JoinAt {
        nick: String,
        addr: String,
        port: u16,
    },
}

/// An online link: the netplay connection, and what keeps it open the while: the relay session
/// it came through and the room put up in the lobby.
struct OnlineLink {
    link: NetplayLink,
    _relay: Option<RelayHost>,
    _room: Option<Announcer>,
}

impl LinkChannel for OnlineLink {
    fn send(&mut self, flags: i32, buf: &[u8]) {
        self.link.send(flags, buf);
    }

    fn try_recv(&mut self) -> Option<Vec<u8>> {
        self.link.try_recv()
    }

    fn is_closed(&self) -> bool {
        self.link.is_closed()
    }

    fn send_end(&mut self) {
        self.link.send_end();
    }

    fn peer_ended(&self) -> bool {
        self.link.peer_ended()
    }
}

fn online_fail(e: &NetplayFail) -> LinkFail {
    match e {
        NetplayFail::Full | NetplayFail::Banned | NetplayFail::Password | NetplayFail::Refused => {
            LinkFail::Refused
        }
        NetplayFail::Core(_) => LinkFail::OtherCore,
        NetplayFail::TimedOut => LinkFail::NobodyCame,
        NetplayFail::Probe | NetplayFail::Protocol(_) | NetplayFail::Io(_) => {
            LinkFail::PeerVanished
        }
    }
}

impl LinkStarter {
    /// Starts `job` online, through RetroArch's lobby at `lobby` and its relays.
    pub fn spawn_online(job: OnlineJob, lobby: &str) -> LinkStarter {
        let (tx, rx) = channel();
        let cancel = Cancel::new();
        let flag = cancel.clone();
        let lobby = lobby.to_string();
        std::thread::Builder::new()
            .name("omega-online".into())
            .spawn(move || {
                let outcome = match job {
                    OnlineJob::Host {
                        nick,
                        relay,
                        game,
                        crc,
                        rom,
                    } => {
                        let crc = rom
                            .and_then(|rom| std::fs::read(rom).ok())
                            .map_or(crc, |bytes| crate::label_sync::crc32(&bytes));
                        host_online(&tx, &flag, &lobby, &nick, &relay, game, crc)
                    }
                    OnlineJob::Join { nick, room } => {
                        let _ = tx.send(LinkProgress::At(LinkStep::Reaching));
                        let joined = match room.relay() {
                            Some((addr, port, session)) => {
                                netplay::join_relay(&addr, port, session, &nick, ANSWER)
                            }
                            None => direct(&room.ip, room.port)
                                .and_then(|s| netplay::join(s, &nick, ANSWER)),
                        };
                        joined.map(|j| ready(j, None, None)).map_err(|e| {
                            eprintln!("omega-zero: netplay: the room: {e}");
                            online_fail(&e)
                        })
                    }
                    OnlineJob::JoinAt { nick, addr, port } => {
                        let _ = tx.send(LinkProgress::At(LinkStep::Reaching));
                        direct(&addr, port)
                            .and_then(|s| netplay::join(s, &nick, ANSWER))
                            .map(|j| ready(j, None, None))
                            .map_err(|e| {
                                eprintln!("omega-zero: netplay: {addr}:{port}: {e}");
                                online_fail(&e)
                            })
                    }
                };
                let progress = match outcome {
                    Ok(connected) => LinkProgress::Ready(connected),
                    Err(_) if flag.is_cancelled() => LinkProgress::Failed(LinkFail::Cancelled),
                    Err(fail) => LinkProgress::Failed(fail),
                };
                let _ = tx.send(progress);
            })
            .expect("the online link's thread");
        LinkStarter {
            rx,
            cancel,
            done: false,
        }
    }
}

fn ready(j: netplay::Joined, relay: Option<RelayHost>, room: Option<Announcer>) -> Connected {
    Connected {
        link: Box::new(OnlineLink {
            link: j.link,
            _relay: relay,
            _room: room,
        }),
        client: Some(j.client),
        peer: Some(j.peer),
    }
}

fn direct(addr: &str, port: u16) -> Result<std::net::TcpStream, NetplayFail> {
    use std::net::ToSocketAddrs;
    let mut last = None;
    for at in (addr, port).to_socket_addrs()? {
        match std::net::TcpStream::connect_timeout(&at, ANSWER) {
            Ok(s) => return Ok(s),
            Err(e) => last = Some(e),
        }
    }
    Err(NetplayFail::Io(last.unwrap_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("no address for {addr}"),
        )
    })))
}

/// Opens a room through the relay called `relay`, puts it up in the lobby, and waits for a
/// player until [`ROOM_OPEN`] has passed or the link is cancelled.
fn host_online(
    tx: &Sender<LinkProgress>,
    cancel: &Cancel,
    lobby: &str,
    nick: &str,
    relay: &str,
    game: String,
    crc: u32,
) -> Result<Connected, LinkFail> {
    let _ = tx.send(LinkProgress::At(LinkStep::Room));
    let (addr, port) = netplay_lobby::tunnel(lobby, relay).map_err(|e| {
        eprintln!("omega-zero: netplay: the relay {relay}: {e}");
        LinkFail::Lobby
    })?;
    let hosting = RelayHost::open(&addr, port, ANSWER).map_err(|e| {
        eprintln!("omega-zero: netplay: the relay at {addr}:{port}: {e}");
        LinkFail::Lobby
    })?;
    let room = Announcer::start(
        lobby,
        Announce {
            username: nick.to_string(),
            game_name: game,
            game_crc: crc,
            relay: relay.to_string(),
            session: hosting.session,
        },
    );
    let _ = tx.send(LinkProgress::At(LinkStep::Waiting));
    let until = Instant::now() + ROOM_OPEN;
    while Instant::now() < until {
        if cancel.is_cancelled() {
            return Err(LinkFail::Cancelled);
        }
        let Some(player) = hosting.next_player(Duration::from_millis(250)) else {
            continue;
        };
        match netplay::host(player, nick, crc, ANSWER) {
            Ok(joined) => {
                eprintln!("omega-zero: netplay: {} joined the room", joined.peer);
                room.set_players(2);
                return Ok(ready(joined, Some(hosting), Some(room)));
            }
            // the lobby looking in, or a player who did not finish: the room stays open
            Err(NetplayFail::Probe) => {}
            Err(e) => eprintln!("omega-zero: netplay: a player could not join: {e}"),
        }
    }
    Err(LinkFail::NobodyCame)
}

/// The joinable rooms in the lobby at `lobby`, fetched on a thread of their own.
pub struct RoomsFetch {
    rx: Receiver<Result<Vec<Room>, ()>>,
}

impl RoomsFetch {
    pub fn spawn(lobby: &str) -> RoomsFetch {
        let (tx, rx) = channel();
        let lobby = lobby.to_string();
        let _ = std::thread::Builder::new()
            .name("omega-rooms".into())
            .spawn(move || {
                let rooms = netplay_lobby::rooms(&lobby)
                    .map(|all| all.into_iter().filter(Room::joinable).collect())
                    .map_err(|e| eprintln!("omega-zero: netplay: the lobby's rooms: {e}"));
                let _ = tx.send(rooms);
            });
        RoomsFetch { rx }
    }

    /// The rooms once they have come: none while they are on their way, an error when the lobby
    /// could not be reached.
    pub fn poll(&self) -> Option<Result<Vec<Room>, ()>> {
        match self.rx.try_recv() {
            Ok(rooms) => Some(rooms),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err(())),
        }
    }
}

#[cfg(test)]
mod port_tests {
    use super::*;

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn lock() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn unset_means_the_number_both_devices_already_agree_on() {
        let _g = lock();
        std::env::remove_var(PORT_ENV);
        assert_eq!(link_port(), DEFAULT_LINK_PORT);
    }

    #[test]
    fn a_port_in_the_environment_wins() {
        let _g = lock();
        std::env::set_var(PORT_ENV, "7300");
        assert_eq!(link_port(), 7300);
        std::env::remove_var(PORT_ENV);
    }

    #[test]
    fn a_padded_port_is_still_a_port() {
        let _g = lock();
        std::env::set_var(PORT_ENV, "  7300 ");
        assert_eq!(link_port(), 7300);
        std::env::remove_var(PORT_ENV);
    }

    #[test]
    fn a_value_that_is_not_a_port_falls_back() {
        let _g = lock();
        for bad in ["banana", "-1", "70000", "7300x"] {
            std::env::set_var(PORT_ENV, bad);
            assert_eq!(
                link_port(),
                DEFAULT_LINK_PORT,
                "{bad:?} should not be taken"
            );
        }
        std::env::remove_var(PORT_ENV);
    }

    #[test]
    fn an_empty_value_is_an_unset_value() {
        let _g = lock();
        std::env::set_var(PORT_ENV, "   ");
        assert_eq!(link_port(), DEFAULT_LINK_PORT);
        std::env::remove_var(PORT_ENV);
    }

    #[test]
    fn zero_is_refused_even_though_it_parses() {
        let _g = lock();
        std::env::set_var(PORT_ENV, "0");
        assert_eq!(link_port(), DEFAULT_LINK_PORT);
        std::env::remove_var(PORT_ENV);
    }
}

#[cfg(test)]
mod step_tests {
    use super::*;

    #[test]
    fn a_warm_radio_captions_the_first_step_as_the_search() {
        assert_eq!(LinkStep::Radio.shown(true), LinkStep::Waiting);
        assert_eq!(
            LinkStep::Radio.shown(true).line(),
            "Looking for the other player"
        );
    }

    #[test]
    fn a_cold_radio_still_says_it_is_bringing_the_radio_up() {
        assert_eq!(LinkStep::Radio.shown(false), LinkStep::Radio);
        assert_eq!(LinkStep::Radio.shown(false).line(), "Bringing the radio up");
    }

    #[test]
    fn the_socket_step_says_the_same_thing_in_both_states() {
        assert_eq!(LinkStep::Waiting.shown(true), LinkStep::Waiting);
        assert_eq!(LinkStep::Waiting.shown(false), LinkStep::Waiting);
    }

    #[test]
    fn every_sentence_a_step_can_show_is_still_one_of_the_faces() {
        for warm in [true, false] {
            for step in LinkStep::ALL {
                assert!(
                    LinkStep::ALL.contains(&step.shown(warm)),
                    "{step:?} at warm={warm} shows a sentence with no face uploaded for it"
                );
            }
        }
    }
}
