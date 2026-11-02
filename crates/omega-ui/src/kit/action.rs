//! The bottom bar's actions, as assets/ui/components/action-left.js and action-right.js: the
//! game's STATUS / L1 BUTTON on the left and R1 BUTTON / ELF LIST on the right. Left to right,
//! the left one is the arrow in its notch at the screen's edge, a big word, and a button pill
//! with a hint; the right one is the same mirrored, parts and all.
//!
//! It is `BAR` art pixels tall and as wide as its text, with room kept when there is none. The
//! notch rises `RISE` pixels above it, over whatever is there, as the game's does over the
//! backdrop, so the face is `BAR + RISE` tall and clear above the bar beside the notch. A bar's
//! two actions are made together ([`action_pair`]) so they come out the same width, each word
//! and each hint centred in the room of the wider one.

use super::palette::Palette;
use super::pixel::{
    big_word, blank, fill, flip, ink, letters, nine_slice, paint, text_width, ADVANCE,
};
use crate::plate::UndoFace;

pub use super::pixel::Font;

/// The bar's height in art pixels.
pub const BAR: u32 = 15;
/// How far the notch rises above the bar.
pub const RISE: u32 = 4;
/// The notch's width.
pub const NOTCH: u32 = 11;
/// The face's height, the notch's.
pub const TALL: u32 = BAR + RISE;
/// Where the bracket over the notch starts down toward the bar's top rail.
const KNEE: u32 = 3;

/// A return arrow: a big head at the top pointing in, toward the word, on a thick band that runs
/// back, turns down and comes round underneath.
#[rustfmt::skip]
const ARROW: [&str; 9] = [
    ".....#..",
    ".....##.",
    "..######",
    ".#######",
    "##...##.",
    "##...#..",
    "##......",
    ".#####..",
    "..####..",
];

/// The button pill, the bar's full height: its left edge, the column that stretches (frame
/// colour on top, a band, a frame line, dark, frame colour at the bottom), then the rounded end.
#[rustfmt::skip]
const PILL: [&str; BAR as usize] = [
    "AAA...",
    "AbbAA.",
    "AAAAAA",
    "AkkkkA",
    "AkkkkA",
    "AkkkkA",
    "AkkkkA",
    "AkkkkA",
    "AkkkkA",
    "AkkkkA",
    "AkkkkA",
    "AkkkkA",
    "AkkkkA",
    "AkkAA.",
    "AAA...",
];
/// How wide the pill's rounded end is.
const PILL_END: u32 = 4;

/// The room either side of the word: 3 dark pixels before it, 2 after its shadow column.
const WORD_PAD: [u32; 2] = [3, 2];
/// How wide the word's box is kept when there is no word.
const WORD_MIN: u32 = 32;

/// The hint's box: never narrower than this, with this room before and after its text.
const HINT_MIN: u32 = 30;
const HINT_PAD: [u32; 2] = [3, 2];
/// How far the hint's box reaches into the pill's rounded end, and how far down the bar its
/// line starts: its capitals fill rows 5 to 11, two dark rows each side between the band and
/// the bottom.
const HINT_REACH: u32 = 3;
const HINT_TOP: u32 = 3;

/// Which end of the bar an action stands at.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Side {
    Left,
    Right,
}

/// An action's face.
pub struct ActionFace {
    pub face: UndoFace,
    /// How far down the face the bar starts: the face goes on the screen this far above it.
    pub rise: u32,
}

/// The notch as rows of colour keys, drawn for the left action: the bracket over it, the dark
/// inside, the arrow over its shadow.
fn notch() -> Vec<String> {
    let (tall, notch, knee, rise) = (TALL as usize, NOTCH as usize, KNEE as usize, RISE as usize);
    let mut grid = vec![vec!['.'; notch]; tall];
    let run = |grid: &mut Vec<Vec<char>>, y: usize, from: usize, to: usize, key: char| {
        let to = to.min(notch);
        if from < to {
            grid[y][from..to].fill(key);
        }
    };
    // the bracket's frame line: along the top, down 45 degrees, then along the bar's rail
    let line = |r: usize| match r {
        0 => (0, knee + 1),
        r if r < rise => (knee + r, knee + r + 2),
        _ => (knee + rise, notch),
    };
    for y in 2..tall - 1 {
        let stripe_from = match y {
            y if y <= rise => knee + y - 1,
            y if y == rise + 1 => knee + rise,
            _ => notch,
        };
        run(&mut grid, y, 0, stripe_from, 'k');
    }
    for r in 0..=rise {
        let (from, to) = line(r);
        run(&mut grid, r, from, to, 'A');
    }
    // the stripe under it: where the line was a row above and is not on this row
    for (r, row) in grid.iter_mut().enumerate().take(rise + 2).skip(1) {
        let (from, to) = line(r - 1);
        for cell in &mut row[from..to] {
            if *cell != 'A' {
                *cell = 's';
            }
        }
    }
    run(&mut grid, tall - 1, 0, notch, 'A');

    // the arrow over its own shadow, one pixel down and right, for some depth, in the middle of
    // the bar between its rail and its bottom line
    for (dx, dy, key) in [(1, 1, 's'), (0, 0, 'H')] {
        for (y, row) in ARROW.iter().enumerate() {
            for (x, ch) in row.chars().enumerate() {
                if ch == '#' {
                    grid[rise + 3 + y + dy][1 + x + dx] = key;
                }
            }
        }
    }
    grid.into_iter()
        .map(|row| row.into_iter().collect())
        .collect()
}

/// How wide the word's box is with `word` in it: the word and its room, or `WORD_MIN`.
fn word_box(word: &str, font: Font) -> u32 {
    match word {
        "" => WORD_MIN,
        _ => WORD_PAD[0] + big_word(word, font)[0].len() as u32 + WORD_PAD[1],
    }
}

/// How wide the hint's box is with `pill` in it.
fn hint_box(pill: &str) -> u32 {
    HINT_MIN.max(HINT_PAD[0] + text_width(pill) + HINT_PAD[1])
}

/// How wide an action is in art pixels with `word` in `font` and `pill`, at either side.
pub fn action_width(word: &str, pill: &str, font: Font) -> u32 {
    NOTCH + word_box(word, font) + button_width(hint_box(pill))
}

/// The pill's width around a hint's box `hint_w` wide: its edge, the box less what reaches into
/// the rounded end, the end.
fn button_width(hint_w: u32) -> u32 {
    1 + hint_w - HINT_REACH + PILL_END
}

/// The action at `side` of the bar: `word` big in `font` beside the arrow, `pill` small in the
/// button. Either may be empty, which keeps their room.
pub fn action_face(side: Side, word: &str, pill: &str, font: Font, pal: &Palette) -> ActionFace {
    face_in(
        side,
        word,
        pill,
        font,
        [word_box(word, font), hint_box(pill)],
        pal,
    )
}

/// A bar's two actions, `left` and `right` as (word, pill), their words in `font`, the same
/// size: each word's box as wide as the wider of the two, and each pill as wide as the wider,
/// the words and the hints centred in them.
pub fn action_pair(
    left: (&str, &str),
    right: (&str, &str),
    font: Font,
    pal: &Palette,
) -> [ActionFace; 2] {
    let room = [
        word_box(left.0, font).max(word_box(right.0, font)),
        hint_box(left.1).max(hint_box(right.1)),
    ];
    [
        face_in(Side::Left, left.0, left.1, font, room, pal),
        face_in(Side::Right, right.0, right.1, font, room, pal),
    ]
}

/// The action at `side` with its word's box `word_w` and its hint's box `hint_w` wide.
fn face_in(
    side: Side,
    word: &str,
    pill: &str,
    font: Font,
    [word_w, hint_w]: [u32; 2],
    pal: &Palette,
) -> ActionFace {
    let w = NOTCH + word_w + button_width(hint_w);
    let mut face = blank(w, TALL);
    let (word_w, button_w) = (word_w as i32, button_width(hint_w) as i32);
    let (bar, rise) = (BAR as i32, RISE as i32);
    let (button_x, word_x, notch_x) = match side {
        Side::Left => (NOTCH as i32 + word_w, NOTCH as i32, 0),
        Side::Right => (0, button_w, button_w + word_w),
    };

    let notch = match side {
        Side::Left => notch(),
        Side::Right => flip(&notch()),
    };
    paint(&mut face, &notch, notch_x, 0, pal);

    // the word's box: the dark of the bar, its top rail (frame colour over a stripe) and its
    // bottom line, then the word with its top on the rail
    fill(&mut face, [word_x, rise, word_w, bar], pal.dark);
    fill(&mut face, [word_x, rise + 1, word_w, 1], pal.stripe);
    fill(&mut face, [word_x, rise, word_w, 1], pal.frame);
    fill(&mut face, [word_x, rise + bar - 1, word_w, 1], pal.frame);
    if !word.is_empty() {
        paint(
            &mut face,
            &big_word(word, font),
            word_x + WORD_PAD[0] as i32 + (word_w - word_box(word, font) as i32) / 2,
            rise + 3,
            pal,
        );
    }

    // the pill, its rounded end toward the word, and the hint in the middle of its dark inside
    let (rows, slice) = match side {
        Side::Left => (Vec::from(PILL.map(String::from)), [0, PILL_END, 0, 1]),
        Side::Right => (flip(&PILL), [0, 1, 0, PILL_END]),
    };
    let button = [button_x, rise, button_w, bar];
    nine_slice(&mut face, &rows, button, slice, slice, pal);
    let hint_x = match side {
        Side::Left => button_x + 1,
        Side::Right => button_x + PILL_END as i32 - HINT_REACH as i32,
    };
    // centred, what is left over split with the odd pixel on the right, so the hint never lands
    // on half a pixel (action-left.js places it the same way from its `room`)
    let room = hint_w - HINT_PAD[0] - HINT_PAD[1];
    let centred = (room.saturating_sub(text_width(pill)) / 2) as i32;
    let text_x = hint_x + HINT_PAD[0] as i32 + centred;
    ink(
        &mut face,
        &letters(pill, ADVANCE),
        text_x,
        rise + HINT_TOP as i32,
        pal.cream,
    );

    ActionFace { face, rise: RISE }
}

/// An action standing between the bar's two, on its row: `pill` in the button and `word` big in
/// `font` beside it, as the right action has them, without the notch, the word's box closed by
/// a frame line where the notch would be. `BAR` tall. The save states' X DELETE is one.
pub fn middle_action_face(word: &str, pill: &str, font: Font, pal: &Palette) -> UndoFace {
    let room = [word_box(word, font), hint_box(pill)];
    let whole = face_in(Side::Right, word, pill, font, room, pal).face;
    let w = whole.w - NOTCH + 1;
    let mut face = blank(w, BAR);
    for y in 0..BAR {
        let from = (((y + RISE) * whole.w) * 4) as usize;
        let to = ((y * w) * 4) as usize;
        face.rgba[to..to + (w * 4) as usize]
            .copy_from_slice(&whole.rgba[from..from + (w * 4) as usize]);
    }
    fill(&mut face, [w as i32 - 1, 0, 1, BAR as i32], pal.frame);
    // the pill's end squared off as the word's box is: its rounded columns as its straight
    // middle, behind a frame line
    for y in 0..BAR {
        let row = (y * w * 4) as usize;
        let middle = row + (PILL_END * 4) as usize;
        let straight: [u8; 4] = face.rgba[middle..middle + 4].try_into().unwrap_or([0; 4]);
        for x in 1..PILL_END {
            let at = row + (x * 4) as usize;
            face.rgba[at..at + 4].copy_from_slice(&straight);
        }
    }
    fill(&mut face, [0, 0, 1, BAR as i32], pal.frame);
    face
}

/// The bar's button pill standing on its own, both its ends rounded, `text` in its dark middle
/// on the rows the buttons print theirs on: the shelf's system, "GBA", between the actions. It
/// is the bar's full height, and has 3 dark pixels either side of the text's ink.
pub fn pill_face(text: &str, pal: &Palette) -> UndoFace {
    let end = PILL_END as usize;
    let rows: Vec<String> = PILL
        .iter()
        .map(|row| {
            let right = &row[2..2 + end];
            let left: String = right.chars().rev().collect();
            format!("{left}{}{right}", &row[1..2])
        })
        .collect();
    // the ink, then its blank column and 2 more; 3 before it, past the end's frame column
    let inner = 3 + text_width(text) + 2;
    let w = inner + 2;
    let mut face = blank(w, BAR);
    let slice = [0, PILL_END, 0, PILL_END];
    nine_slice(
        &mut face,
        &rows,
        [0, 0, w as i32, BAR as i32],
        slice,
        slice,
        pal,
    );
    ink(
        &mut face,
        &letters(text, ADVANCE),
        4,
        HINT_TOP as i32,
        pal.cream,
    );
    face
}
