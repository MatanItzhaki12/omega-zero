use std::io;
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use omega_zero::link_net::{Cancel, TcpLink};
use omega_zero::link_radio::{LinkRole, RadioFail};
use omega_zero::link_start::{LinkFail, LinkProgress, LinkStarter, LinkStep};

const BAIL: Duration = Duration::from_secs(5);

fn drain(starter: &mut LinkStarter) -> LinkProgress {
    let deadline = Instant::now() + BAIL;
    loop {
        match starter.poll() {
            Some(LinkProgress::At(_)) => {}
            Some(outcome) => return outcome,
            None => std::thread::sleep(Duration::from_millis(2)),
        }
        assert!(
            Instant::now() < deadline,
            "the worker never reached an outcome"
        );
    }
}

fn drain_steps(starter: &mut LinkStarter) -> Vec<LinkStep> {
    let deadline = Instant::now() + BAIL;
    let mut steps = Vec::new();
    loop {
        match starter.poll() {
            Some(LinkProgress::At(step)) => steps.push(step),
            Some(_) => return steps,
            None => std::thread::sleep(Duration::from_millis(2)),
        }
        assert!(
            Instant::now() < deadline,
            "the worker never reached an outcome"
        );
    }
}

#[test]
fn a_radio_that_will_not_come_up_stops_before_the_socket() {
    let tried_socket = Arc::new(AtomicBool::new(false));
    let seen = tried_socket.clone();
    let downs = Arc::new(AtomicUsize::new(0));
    let count = downs.clone();
    let mut starter = LinkStarter::spawn_with(
        Box::new(|_role, _| Err(RadioFail::Radio("no ap".into()))),
        Box::new(move || {
            count.fetch_add(1, Ordering::SeqCst);
        }),
        LinkRole::Host,
        0,
        Box::new(move |_, _| {
            seen.store(true, Ordering::SeqCst);
            Err(io::Error::other("must not be reached"))
        }),
    );
    let outcome = drain(&mut starter);
    assert!(matches!(outcome, LinkProgress::Failed(LinkFail::Radio)));
    assert!(
        !tried_socket.load(Ordering::SeqCst),
        "opened a socket on a network that never came up"
    );
    assert_eq!(
        downs.load(Ordering::SeqCst),
        1,
        "a radio that only half came up must still be taken down"
    );
}

#[test]
fn a_failure_always_takes_the_radio_back_down() {
    let downs = Arc::new(AtomicUsize::new(0));
    let count = downs.clone();
    let mut starter = LinkStarter::spawn_with(
        Box::new(|_, _| Ok(())),
        Box::new(move || {
            count.fetch_add(1, Ordering::SeqCst);
        }),
        LinkRole::Host,
        0,
        Box::new(|_, _| Err(io::Error::new(io::ErrorKind::TimedOut, "nobody"))),
    );
    let outcome = drain(&mut starter);
    assert!(matches!(
        outcome,
        LinkProgress::Failed(LinkFail::NobodyCame)
    ));
    assert_eq!(
        downs.load(Ordering::SeqCst),
        1,
        "a failed link must never leave the radio up"
    );
    assert!(
        starter.poll().is_none(),
        "the worker said how it ended; there is nothing after that"
    );
}

#[test]
fn a_link_that_comes_up_leaves_the_radio_up() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let downs = Arc::new(AtomicUsize::new(0));
    let count = downs.clone();
    let peer = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        TcpLink::join("127.0.0.1", port)
    });
    let mut starter = LinkStarter::spawn_with(
        Box::new(|_, _| Ok(())),
        Box::new(move || {
            count.fetch_add(1, Ordering::SeqCst);
        }),
        LinkRole::Host,
        port,
        Box::new(|port, cancel: &Cancel| {
            TcpLink::host_until("127.0.0.1", port, Duration::from_secs(10), cancel)
        }),
    );
    let outcome = drain(&mut starter);
    assert!(
        matches!(outcome, LinkProgress::Ready(_)),
        "the peer arrived inside the bound, so this is a link"
    );
    let _joiner = peer.join().unwrap().expect("joiner connected");
    assert_eq!(
        downs.load(Ordering::SeqCst),
        0,
        "tearing the radio down on success kills the session it was brought up for"
    );
    assert!(
        starter.poll().is_none(),
        "a worker that has handed over its link must not then invent a failure"
    );
}

#[test]
fn the_steps_are_reported_in_order_before_the_outcome() {
    let mut starter = LinkStarter::spawn_with(
        Box::new(|_, _| Ok(())),
        Box::new(|| {}),
        LinkRole::Host,
        0,
        Box::new(|_, _| Err(io::Error::new(io::ErrorKind::TimedOut, "nobody"))),
    );
    let steps = drain_steps(&mut starter);
    assert_eq!(steps, vec![LinkStep::Radio, LinkStep::Waiting]);
}

#[test]
fn cancelling_reports_cancelled_rather_than_a_timeout() {
    let mut starter = LinkStarter::spawn_with(
        Box::new(|_, _| Ok(())),
        Box::new(|| {}),
        LinkRole::Host,
        0,
        Box::new(|_, cancel: &Cancel| {
            while !cancel.is_cancelled() {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"))
        }),
    );
    starter.cancel();
    let outcome = drain(&mut starter);
    assert!(
        matches!(outcome, LinkProgress::Failed(LinkFail::Cancelled)),
        "a player who backed out is not a player nobody joined"
    );
}

#[test]
fn a_socket_fault_that_is_neither_a_deadline_nor_a_cancel_blames_the_wire() {
    let mut starter = LinkStarter::spawn_with(
        Box::new(|_, _| Ok(())),
        Box::new(|| {}),
        LinkRole::Join,
        0,
        Box::new(|_, _| Err(io::Error::new(io::ErrorKind::ConnectionRefused, "refused"))),
    );
    assert!(matches!(
        drain(&mut starter),
        LinkProgress::Failed(LinkFail::PeerVanished)
    ));
}

#[test]
fn a_worker_that_dies_is_reported_rather_than_polled_forever() {
    let mut starter = LinkStarter::spawn_with(
        Box::new(|_, _| panic!("the radio call blew up")),
        Box::new(|| {}),
        LinkRole::Host,
        0,
        Box::new(|_, _| Err(io::Error::other("must not be reached"))),
    );
    assert!(matches!(
        drain(&mut starter),
        LinkProgress::Failed(LinkFail::PeerVanished)
    ));
}

#[test]
fn a_link_that_comes_up_after_the_player_left_puts_the_radio_back() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let peer = std::thread::spawn(move || {
        let accepted = listener.accept();
        std::thread::sleep(Duration::from_millis(300));
        drop(accepted);
    });

    let downs = Arc::new(AtomicUsize::new(0));
    let count = downs.clone();
    let gate = Arc::new(AtomicBool::new(false));
    let open = gate.clone();

    let starter = LinkStarter::spawn_with(
        Box::new(|_, _| Ok(())),
        Box::new(move || {
            count.fetch_add(1, Ordering::SeqCst);
        }),
        LinkRole::Join,
        port,
        Box::new(move |p, _| {
            while !open.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(5));
            }
            TcpLink::join("127.0.0.1", p)
        }),
    );

    drop(starter);
    gate.store(true, Ordering::SeqCst);

    let deadline = Instant::now() + Duration::from_secs(5);
    while downs.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        downs.load(Ordering::SeqCst),
        1,
        "a link nobody was left to receive must still put the radio back"
    );
    let _ = peer.join();
}

#[test]
fn a_join_that_found_no_host_says_nobody_arrived() {
    let tried_socket = Arc::new(AtomicBool::new(false));
    let seen = tried_socket.clone();
    let mut starter = LinkStarter::spawn_with(
        Box::new(|_, _| Err(RadioFail::NoHost)),
        Box::new(|| {}),
        LinkRole::Join,
        0,
        Box::new(move |_, _| {
            seen.store(true, Ordering::SeqCst);
            Err(io::Error::other("must not be reached"))
        }),
    );
    assert!(matches!(
        drain(&mut starter),
        LinkProgress::Failed(LinkFail::NobodyCame)
    ));
    assert!(
        !tried_socket.load(Ordering::SeqCst),
        "there was no host to reach, so there is nothing to open a socket to"
    );
}

#[test]
fn a_cancel_while_the_radio_is_coming_up_is_not_a_fault() {
    let mut starter = LinkStarter::spawn_with(
        Box::new(|_, cancel: &Cancel| {
            while !cancel.is_cancelled() {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(RadioFail::Cancelled)
        }),
        Box::new(|| {}),
        LinkRole::Join,
        0,
        Box::new(|_, _| Err(io::Error::other("must not be reached"))),
    );
    starter.cancel();
    assert!(matches!(
        drain(&mut starter),
        LinkProgress::Failed(LinkFail::Cancelled)
    ));
}

#[test]
fn a_starter_that_is_dropped_stops_waiting() {
    let gave_up = Arc::new(AtomicBool::new(false));
    let noticed = gave_up.clone();
    let starter = LinkStarter::spawn_with(
        Box::new(|_, _| Ok(())),
        Box::new(|| {}),
        LinkRole::Host,
        0,
        Box::new(move |_, cancel| {
            let deadline = Instant::now() + BAIL;
            while !cancel.is_cancelled() {
                assert!(
                    Instant::now() < deadline,
                    "the dropped starter was never told to give up"
                );
                std::thread::sleep(Duration::from_millis(2));
            }
            noticed.store(true, Ordering::SeqCst);
            Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"))
        }),
    );

    drop(starter);

    let deadline = Instant::now() + BAIL;
    while !gave_up.load(Ordering::SeqCst) {
        assert!(
            Instant::now() < deadline,
            "the worker kept waiting for a peer nobody was left to play with"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

// ---- online ----

use std::io::{Read, Write};
use std::sync::Mutex;

use omega_zero::link_start::OnlineJob;

/// A lobby answering /tunnel with the relay at `relay` and /add with a room's id, keeping every
/// form it is sent.
fn fake_lobby(relay: u16) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let forms = Arc::new(Mutex::new(Vec::new()));
    let kept = forms.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut asked = Vec::new();
            let mut byte = [0u8; 1];
            while !asked.ends_with(b"\r\n\r\n") && stream.read(&mut byte).unwrap_or(0) == 1 {
                asked.push(byte[0]);
            }
            let head = String::from_utf8_lossy(&asked).to_string();
            let length = head
                .lines()
                .find_map(|l| {
                    l.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(|v| v.trim().to_string())
                })
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(0);
            let mut body = vec![0u8; length];
            let _ = stream.read_exact(&mut body);
            let said = if head.starts_with("GET /tunnel") {
                format!("tunnel_addr=127.0.0.1\ntunnel_port={relay}\n")
            } else {
                kept.lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&body).to_string());
                "id=1\nconnectable=1\n".to_string()
            };
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{said}",
                said.len()
            );
        }
    });
    (format!("http://127.0.0.1:{port}/"), forms)
}

fn relay_id(magic: &[u8; 4], id: &[u8; 12]) -> [u8; 16] {
    let mut out = [0u8; 16];
    out[..4].copy_from_slice(magic);
    out[4..].copy_from_slice(id);
    out
}

/// A room opened online: its relay found in the lobby, a session taken there, the room put up
/// with that session, and a player the relay brings let in as the one who joins.
#[test]
fn a_room_opened_online_is_put_up_and_takes_the_player_its_relay_brings() {
    let relay = TcpListener::bind("127.0.0.1:0").unwrap();
    let relay_port = relay.local_addr().unwrap().port();
    let (lobby, forms) = fake_lobby(relay_port);
    let session = [5u8; 12];
    let fake_relay = std::thread::spawn(move || {
        let (mut control, _) = relay.accept().unwrap();
        let mut ask = [0u8; 16];
        control.read_exact(&mut ask).unwrap();
        control.write_all(&relay_id(b"RATS", &session)).unwrap();
        // the room is in the lobby before a player is brought
        std::thread::sleep(Duration::from_millis(300));
        let link = [6u8; 12];
        control.write_all(&relay_id(b"RATL", &link)).unwrap();
        let (mut theirs, _) = relay.accept().unwrap();
        let mut addr = [0u8; 16];
        control.read_exact(&mut addr).unwrap();
        let mut answer = relay_id(b"RATA", &link).to_vec();
        answer.extend([0u8; 16]);
        control.write_all(&answer).unwrap();
        let mut claim = [0u8; 16];
        theirs.read_exact(&mut claim).unwrap();
        let joined = omega_zero::netplay::join(theirs, "Guest", BAIL).expect("the player's join");
        (control, joined)
    });
    let mut starter = LinkStarter::spawn_online(
        OnlineJob::Host {
            nick: "RG SP".into(),
            relay: "madrid".into(),
            game: "Pokemon - Emerald Version (USA, Europe)".into(),
            crc: 0x1F1C_08FB,
            rom: None,
        },
        &lobby,
    );
    let steps = drain_steps(&mut starter);
    assert_eq!(steps, vec![LinkStep::Room, LinkStep::Waiting]);
    let (_control, joined) = fake_relay.join().unwrap();
    assert_eq!(joined.peer, "RG SP");
    let forms = forms.lock().unwrap();
    let room = forms.first().expect("the room was never put up");
    for field in [
        "mitm_session=BQUFBQUFBQUFBQUF",
        "mitm_server=madrid",
        "game_crc=1F1C08FB",
        "username=RG%20SP",
    ] {
        assert!(
            room.split('&').any(|f| f == field),
            "{field} missing from {room}"
        );
    }
}

/// The ready link of a room opened online plays as client 0, and names who joined.
#[test]
fn a_room_joined_at_an_address_plays_as_the_client_the_host_numbers_it() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let host = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        omega_zero::netplay::host(stream, "RetroArch", 0, BAIL)
    });
    let mut starter = LinkStarter::spawn_online(
        OnlineJob::JoinAt {
            nick: "RG SP".into(),
            addr: "127.0.0.1".into(),
            port,
        },
        "http://127.0.0.1:9/",
    );
    match drain(&mut starter) {
        LinkProgress::Ready(ready) => {
            assert_eq!(ready.client, Some(1));
            assert_eq!(ready.peer.as_deref(), Some("RetroArch"));
        }
        LinkProgress::Failed(fail) => panic!("the join failed: {fail:?}"),
        LinkProgress::At(_) => unreachable!(),
    }
    let hosted = host.join().unwrap().expect("the host's side");
    assert_eq!(hosted.peer, "RG SP");
}

#[test]
fn a_lobby_that_cannot_be_reached_fails_the_room_as_the_lobby() {
    let closed = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = closed.local_addr().unwrap().port();
    drop(closed);
    let mut starter = LinkStarter::spawn_online(
        OnlineJob::Host {
            nick: "RG SP".into(),
            relay: "madrid".into(),
            game: "Emerald".into(),
            crc: 0,
            rom: None,
        },
        &format!("http://127.0.0.1:{port}/"),
    );
    assert!(matches!(
        drain(&mut starter),
        LinkProgress::Failed(LinkFail::Lobby)
    ));
}
