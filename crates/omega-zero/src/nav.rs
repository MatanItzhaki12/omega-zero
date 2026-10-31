//! Moving between screens. Every move slides: going deeper, the screen left behind leaves to
//! the left as the one it opens comes in from the right; going back, the screen leaves to the
//! right as the one it came from comes back from the left. Settings stands beside the shelf and
//! slides the same way: the shelf leaves to the left as Settings comes in from the right, and
//! going back Settings leaves to the right as the shelf comes back from the left. So the screens
//! under Settings, and the Controls panel under the shelf, carry on the way Settings came in.
//! The backdrop under them never moves.
//!
//! The shelf slides in two layers rather than as one: its carts leave across, the way the
//! slide goes, and its top bar up off the screen, and they come back the same ways.
//!
//! The power screens slide down rather than across (`dropping`): the POWER menu comes down from
//! the top as the screen it was held on drops out of the bottom, the shelf as one with all its
//! layers, and going back is the mirror.

use omega_input::Millis;
use omega_ui::{ease, CART_W, GB_CART_W, SPRITE_PX, TOP_BAR_H};

use crate::app::Phase;

/// How long a slide takes.
pub const SLIDE_MS: Millis = 280;

/// Which way a move goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    /// Into a screen opened from the one shown.
    Deeper,
    /// Back out to the screen the one shown was opened from.
    Back,
    /// Out from the shelf to Settings, beside it: the mirror of going deeper.
    Aside,
    /// Back from Settings to the shelf beside it: the mirror of going back.
    FromAside,
}

/// Which side of a slide a screen is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    Leaving,
    Arriving,
}

/// A slide under way: the screen it leaves, which way it goes and when it started. The screen
/// it arrives at is the app's phase.
#[derive(Clone, Debug)]
pub struct Slide {
    pub from: Phase,
    pub dir: Dir,
    pub started: Millis,
}

impl Slide {
    /// How far it has gone at `now`.
    pub fn progress(&self, now: Millis) -> f32 {
        progress(self.started, now)
    }

    /// It has gone all the way by `now`.
    pub fn over(&self, now: Millis) -> bool {
        now.saturating_sub(self.started) >= SLIDE_MS
    }
}

/// How far a slide that started at `started` has gone at `now`, eased: 0 as it starts and 1
/// once it is over.
pub fn progress(started: Millis, now: Millis) -> f32 {
    ease((now.saturating_sub(started) as f32 / SLIDE_MS as f32).min(1.0))
}

/// `px` rounded to whole art pixels, SPRITE_PX screen pixels each, so a sliding screen keeps
/// its sprites on their grid.
pub fn even(px: f32) -> f32 {
    let art = SPRITE_PX as f32;
    (px / art).round() * art
}

/// The widest a cart is drawn: the neighbour peeking in at the left edge has to travel the
/// screen's width and its own to clear it.
pub fn widest_cart() -> f32 {
    CART_W.max(GB_CART_W) as f32
}

/// How far each layer of a screen is moved while it slides. A menu screen moves as one, `dx`
/// across and `dy` down; the shelf moves its carts `carts_dx` across and `carts_dy` down, its top
/// bar `bar_dy` down (up, being negative).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Offsets {
    pub dx: f32,
    pub dy: f32,
    pub carts_dx: f32,
    pub carts_dy: f32,
    pub bar_dy: f32,
}

impl Offsets {
    /// A screen at rest.
    pub const NONE: Offsets = Offsets {
        dx: 0.0,
        dy: 0.0,
        carts_dx: 0.0,
        carts_dy: 0.0,
        bar_dy: 0.0,
    };
}

/// How far across a screen is that has to travel `span` to clear a canvas, `p` of the way
/// through a slide going `dir`, as it leaves or arrives.
pub fn across(dir: Dir, end: End, p: f32, span: f32) -> f32 {
    // the way the screen left behind goes: left going deeper or out to Settings, right going
    // back or back from Settings
    let rightward = match dir {
        Dir::Back | Dir::FromAside => 1.0,
        Dir::Deeper | Dir::Aside => -1.0,
    };
    match end {
        End::Leaving => rightward * even(p * span),
        End::Arriving => -rightward * even((1.0 - p) * span),
    }
}

/// Where each layer of a screen on a canvas `w` wide is, `p` of the way through a slide going
/// `dir`, as it leaves or arrives. The shelf's layers are as far from home as the slide is
/// from the shelf: all the way gone once it has left, home once it has arrived.
pub fn offsets(dir: Dir, end: End, p: f32, w: f32) -> Offsets {
    let gone = match end {
        End::Leaving => p,
        End::Arriving => 1.0 - p,
    };
    Offsets {
        dx: across(dir, end, p, w),
        carts_dx: across(dir, end, p, w + widest_cart()),
        bar_dy: -even(gone * TOP_BAR_H),
        ..Offsets::NONE
    }
}

/// Where a screen on a canvas `h` tall is, `p` of the way through a slide going `dir` down
/// rather than across, as the power screens slide: going deeper, the screen left behind drops
/// out of the bottom as the one it opens comes down from the top; going back, the screen goes
/// up out of the top as the one it came from rises back from the bottom. The shelf moves as one,
/// its carts and top bar together.
pub fn dropping(dir: Dir, end: End, p: f32, h: f32) -> Offsets {
    // as far down as a slide the same way goes across, the screen left behind going down
    // where it would go left
    let dy = -across(dir, end, p, h);
    Offsets {
        dy,
        carts_dy: dy,
        bar_dy: dy,
        ..Offsets::NONE
    }
}
