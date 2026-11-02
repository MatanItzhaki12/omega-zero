//! Each system's games are played by its own rules on the emulator thread: held X and Y are
//! turbo A and B only on the handhelds, and the PlayStation, whose states are too big, keeps no
//! rewind snapshots.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use omega_retro::{AvInfo, ButtonMask, CoreError, MockCore, RetroCore};
use omega_store::Platform;
use omega_zero::audio::{AudioSink, StubSink};
use omega_zero::emu::{CoreState, EmuHandle, Speed};

/// A mock that writes down what each frame was played with and counts the states taken of it.
struct Recorder {
    inner: MockCore,
    inputs: Arc<Mutex<Vec<u16>>>,
    states: Arc<AtomicUsize>,
}

impl RetroCore for Recorder {
    fn load(&mut self, rom: &Path) -> Result<(), CoreError> {
        self.inner.load(rom)
    }
    fn run_frame(&mut self, input: ButtonMask) {
        self.inputs.lock().unwrap().push(input.0);
        self.inner.run_frame(input);
    }
    fn video_xrgb8888(&self) -> &[u8] {
        self.inner.video_xrgb8888()
    }
    fn video_size(&self) -> (u32, u32) {
        self.inner.video_size()
    }
    fn take_audio(&mut self) -> Vec<i16> {
        self.inner.take_audio()
    }
    fn serialize(&mut self) -> Result<Vec<u8>, CoreError> {
        self.states.fetch_add(1, Ordering::Relaxed);
        self.inner.serialize()
    }
    fn unserialize(&mut self, data: &[u8]) -> Result<(), CoreError> {
        self.inner.unserialize(data)
    }
    fn save_ram(&self) -> Option<Vec<u8>> {
        self.inner.save_ram()
    }
    fn load_save_ram(&mut self, data: &[u8]) -> Result<(), CoreError> {
        self.inner.load_save_ram(data)
    }
    fn av_info(&self) -> AvInfo {
        self.inner.av_info()
    }
}

struct Played {
    emu: EmuHandle,
    inputs: Arc<Mutex<Vec<u16>>>,
    states: Arc<AtomicUsize>,
}

fn play(platform: Platform, held: u16) -> Played {
    let inputs = Arc::new(Mutex::new(Vec::new()));
    let states = Arc::new(AtomicUsize::new(0));
    let core = Box::new(Recorder {
        inner: MockCore::new(),
        inputs: inputs.clone(),
        states: states.clone(),
    });
    let emu = EmuHandle::spawn_for(
        platform,
        core,
        PathBuf::from("mock"),
        StubSink::draining().ring(),
        None,
        None,
        None,
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while emu.state() == CoreState::Loading {
        assert!(Instant::now() < deadline, "the core never settled");
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(emu.state(), CoreState::Ready);
    emu.set_input(ButtonMask(held));
    emu.set_speed(Speed::Normal);
    Played {
        emu,
        inputs,
        states,
    }
}

/// The inputs of the first `n` frames, waiting for them to run.
fn first_frames(p: &Played, n: usize) -> Vec<u16> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let seen = p.inputs.lock().unwrap().clone();
        if seen.len() >= n {
            return seen[..n].to_vec();
        }
        assert!(Instant::now() < deadline, "only {} frames ran", seen.len());
        std::thread::sleep(Duration::from_millis(5));
    }
}

const XY: u16 = ButtonMask::X | ButtonMask::Y;

#[test]
fn on_a_handheld_held_x_and_y_are_turbo_a_and_b() {
    for platform in [Platform::Gba, Platform::Gb, Platform::Gbc] {
        let p = play(platform, XY);
        let frames = first_frames(&p, 12);
        assert!(
            frames.iter().all(|m| m & XY == 0),
            "{platform:?}: X or Y reached the game"
        );
        assert!(
            frames.iter().any(|m| m & ButtonMask::A != 0)
                && frames.iter().any(|m| m & ButtonMask::A == 0),
            "{platform:?}: A did not pulse: {frames:?}"
        );
    }
}

#[test]
fn on_a_console_x_and_y_reach_the_game_as_themselves() {
    for platform in [Platform::Snes, Platform::Ps1] {
        let p = play(platform, XY | ButtonMask::L2 | ButtonMask::R2);
        let frames = first_frames(&p, 12);
        assert!(
            frames
                .iter()
                .all(|&m| m == XY | ButtonMask::L2 | ButtonMask::R2),
            "{platform:?}: the game was not played with X, Y, L2 and R2 as held: {frames:?}"
        );
    }
}

#[test]
fn the_playstation_keeps_no_rewind_snapshots_and_the_snes_does() {
    let ps1 = play(Platform::Ps1, 0);
    let snes = play(Platform::Snes, 0);
    first_frames(&ps1, 30);
    first_frames(&snes, 30);
    assert_eq!(
        ps1.states.load(Ordering::Relaxed),
        0,
        "a PlayStation state was taken for rewind"
    );
    assert!(
        snes.states.load(Ordering::Relaxed) >= 10,
        "a SNES game kept no rewind snapshots"
    );
    ps1.emu.set_rewinding(true);
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(ps1.emu.rewind_fill(), 0);
    drop(ps1);
}

/// Run-ahead keeps a state of every frame, which the PlayStation's are too big for: there it is
/// never taken, with the setting on or off. A handheld's game runs ahead, keeping one a frame.
#[test]
fn the_playstation_never_runs_ahead_and_a_handheld_does() {
    let ps1 = play(Platform::Ps1, 0);
    ps1.emu.set_run_ahead(true);
    first_frames(&ps1, 20);
    assert_eq!(
        ps1.states.load(Ordering::Relaxed),
        0,
        "the PlayStation took states to run ahead"
    );

    // rewind keeps a state every other frame either way; running ahead keeps one every frame
    // more, so over as many frames run, a game running ahead has taken a good few more
    let states = |ahead: bool| {
        let gba = play(Platform::Gba, 0);
        gba.emu.set_run_ahead(ahead);
        first_frames(&gba, 40);
        gba.states.load(Ordering::Relaxed)
    };
    let (plain, ahead) = (states(false), states(true));
    assert!(
        ahead >= plain + 5,
        "{ahead} states running ahead against {plain} without: the GBA did not run ahead"
    );
}
