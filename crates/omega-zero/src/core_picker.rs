//! The core picker: the cart's lid lifted off its board, and the chip hopping between the
//! board's sockets, one for each core that runs the cart's system, in the order the system lists
//! them (`Core::for_platform`). Left and Right move the chip a socket that way; A keeps the core
//! it is in, or heading for, Y hops it to the system's default and keeps that, and B leaves the
//! cart as it was.

use omega_store::Core;
use omega_ui::{ease, Millis, Refusal};

pub const SLIDE_MS: Millis = 160;
pub const LIFT_MS: Millis = 260;
pub const OPEN_MS: Millis = SLIDE_MS + LIFT_MS;
pub const CLOSE_MS: Millis = 320;
pub const HOP_MS: Millis = 180;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Press {
    Left,
    Right,
    Keep,
    Back,
    /// Back to the system's default core.
    Default,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Outcome {
    Nothing,
    Refused,
    Write(Core),
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Chip {
    /// How far across the sockets the chip is, in sockets from the first: a whole number when it
    /// is seated in one.
    pub across: f32,
    pub lift: f32,
    pub seated: Option<Core>,
    pub shake: f32,
}

#[derive(Copy, Clone)]
struct Hop {
    from: usize,
    started: Millis,
}

#[derive(Copy, Clone)]
struct Close {
    started: Millis,
    from: f32,
}

#[derive(Copy, Clone)]
pub struct CorePicker {
    /// The cores the board has a socket for, in socket order.
    cores: &'static [Core],
    /// The socket the chip is in, or heading for.
    slot: usize,
    /// The socket of the system's default core.
    default: usize,
    opened: Millis,
    started: Option<Millis>,
    hop: Option<Hop>,
    close: Option<Close>,
    refusal: Option<Refusal>,
}

impl CorePicker {
    /// A picker over a socket for each of `cores`, the chip in `seat`'s, `default` the system's
    /// default; a core that is none of them is the first.
    pub fn open(cores: &'static [Core], seat: Core, default: Core, now: Millis) -> Self {
        let at = |core: Core| cores.iter().position(|c| *c == core).unwrap_or(0);
        CorePicker {
            cores,
            slot: at(seat),
            default: at(default),
            opened: now,
            started: None,
            hop: None,
            close: None,
            refusal: None,
        }
    }

    pub fn start(&mut self, now: Millis) {
        self.started.get_or_insert(now);
    }

    pub fn waiting(&self) -> bool {
        self.started.is_none()
    }

    pub fn waited(&self, now: Millis) -> Millis {
        now.saturating_sub(self.opened)
    }

    /// The core whose socket the chip is in, or heading for.
    pub fn seat(&self) -> Core {
        self.cores[self.slot]
    }

    /// The cores the board has a socket for, in socket order.
    pub fn cores(&self) -> &'static [Core] {
        self.cores
    }

    pub fn closing(&self) -> bool {
        self.close.is_some()
    }

    pub fn openness(&self, now: Millis) -> f32 {
        match (self.close, self.started) {
            (Some(Close { started, from }), _) => {
                (from - now.saturating_sub(started) as f32 / CLOSE_MS as f32).max(0.0)
            }
            (None, Some(started)) => (now.saturating_sub(started) as f32 / OPEN_MS as f32).min(1.0),
            (None, None) => 0.0,
        }
    }

    pub fn finished(&self, now: Millis) -> bool {
        self.close.is_some() && self.openness(now) <= 0.0
    }

    pub fn press(&mut self, press: Press, now: Millis) -> Outcome {
        if self.close.is_some() {
            return Outcome::Nothing;
        }
        let last = self.cores.len().saturating_sub(1);
        let target = match press {
            Press::Keep => {
                self.begin_close(now);
                return Outcome::Write(self.seat());
            }
            Press::Back => {
                self.begin_close(now);
                return Outcome::Nothing;
            }
            // the chip hops home as the lid comes down
            Press::Default => {
                if !self.waiting() && self.slot != self.default {
                    self.hop = Some(Hop {
                        from: self.slot,
                        started: now,
                    });
                    self.slot = self.default;
                }
                self.begin_close(now);
                return Outcome::Write(self.seat());
            }
            Press::Left => self.slot.saturating_sub(1),
            Press::Right => (self.slot + 1).min(last),
        };
        if self.waiting() {
            return Outcome::Nothing;
        }
        if let Some(progress) = self.hop_progress(now) {
            if target == self.slot {
                return Outcome::Nothing;
            }
            let done = ((1.0 - progress) * HOP_MS as f32) as Millis;
            self.hop = Some(Hop {
                from: self.slot,
                started: now.saturating_sub(done),
            });
            self.slot = target;
            self.refusal = None;
            return Outcome::Nothing;
        }
        if target == self.slot {
            self.refusal = Some(Refusal::started(now));
            return Outcome::Refused;
        }
        self.hop = Some(Hop {
            from: self.slot,
            started: now,
        });
        self.slot = target;
        self.refusal = None;
        Outcome::Nothing
    }

    pub fn chip(&self, now: Millis) -> Chip {
        let shake = self.refusal.map_or(0.0, |r| r.offset(now));
        match (self.hop, self.hop_progress(now)) {
            (Some(hop), Some(q)) => {
                let (from, to) = (hop.from as f32, self.slot as f32);
                let arc = (std::f32::consts::PI * q).sin();
                Chip {
                    across: from + (to - from) * ease(q),
                    lift: arc,
                    seated: None,
                    shake,
                }
            }
            _ => Chip {
                across: self.slot as f32,
                lift: 0.0,
                seated: Some(self.seat()),
                shake,
            },
        }
    }

    fn begin_close(&mut self, now: Millis) {
        self.close = Some(Close {
            started: now,
            from: self.openness(now),
        });
    }

    fn hop_progress(&self, now: Millis) -> Option<f32> {
        let hop = self.hop?;
        let q = now.saturating_sub(hop.started) as f32 / HOP_MS as f32;
        (q < 1.0).then_some(q)
    }
}
