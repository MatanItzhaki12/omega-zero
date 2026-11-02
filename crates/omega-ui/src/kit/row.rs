//! One setting inside a panel's well, as assets/ui/components/omega-row.js: its name on the
//! left and its value on the right. The row the cursor is on is lit the way a panel's title tab
//! is drawn: a band in the tab colour with a frame line round it and its corner pixels left out,
//! the words in it light.
//!
//! A row is `ROW_H` art pixels tall, as tall as a panel's header, and as wide as a panel's well
//! and a pixel more each side, the way omega-panel lays its rows out: [`row_rect`] says where
//! row `i` of a panel goes. Both words are Pixel Code one face pixel to an art pixel, on a line
//! of 10 with their capitals on rows 3 to 9 of the row, three rows clear above and below.
//!
//! Moving the cursor re-bakes two rows, the one it leaves and the one it lands on; a row's
//! carets keep their room unlit, so its value never moves as the cursor passes.

use super::palette::Palette;
use super::panel::HEADER;
use super::pixel::{
    blank, flip, ink, letters, letters_over, mirror, nine_slice, paint, text_width, ADVANCE,
};
use crate::plate::UndoFace;

/// How tall a row is in art pixels.
pub const ROW_H: u32 = 13;

/// How far a row reaches into either side of a panel's well, so a lit row's band has two pixels
/// of well around it on every side, as it has above the first row.
pub const REACH: u32 = 1;

/// The band's frame line, which is the row's border, there lit or not so nothing moves when the
/// cursor arrives; and the room inside it either side of the words.
const BORDER: u32 = 1;
const PAD: u32 = 4;

/// How far the name stays clear of whatever follows it: one letter.
const NAME_GAP: u32 = 6;

/// How far into the row's inside art beside the words (arrows, icons, all 7 tall) starts, so it
/// stands on the capitals' rows.
const ART_TOP: u32 = 2;

/// The band under the cursor, a 9-slice: the frame line, its corner pixel left out like a
/// panel's, around the tab colour.
#[rustfmt::skip]
const BAND: [&str; 2] = [
    ".A",
    "At",
];

/// The ◀ beside a value Left and Right change, as tall as the capitals; '#' is drawn in the
/// arrows' colour. The ▶ is the same turned around.
#[rustfmt::skip]
const CARET: [&str; 7] = [
    "...#",
    "..##",
    ".###",
    "####",
    ".###",
    "..##",
    "...#",
];

/// The › at the end of a row that opens another screen: a chevron two pixels thick, as tall as
/// the capitals and as wide as a caret.
#[rustfmt::skip]
const OPENS: [&str; 7] = [
    "#...",
    "##..",
    ".##.",
    "..##",
    ".##.",
    "##..",
    "#...",
];

/// How wide an arrow is, and how far it stays clear of the value and of the other arrows.
const ARROW_W: u32 = 4;
const ARROW_GAP: i32 = 3;

/// One setting, as omega-row's attributes.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Row<'a> {
    /// The setting's name, from the left, in capitals whatever case it is given in.
    pub label: &'a str,
    /// What it is set to, from the right, as given. A row may have none.
    pub value: Option<&'a str>,
    /// The row under the cursor: the band, the name in cream, the value and arrows bright.
    pub lit: bool,
    /// A ◀ and a ▶ either side of the value while the row is lit, for a value Left and Right
    /// change. Their room is kept unlit too.
    pub carets: bool,
    /// The carets' room kept with no carets drawn, so the value lines up with those of rows
    /// that have them.
    pub caret_room: bool,
    /// A › at the right end, lit or not, for a row that opens another screen.
    pub opens: bool,
    /// A setting that is not available: everything in it drawn faint, lit or not, and no carets
    /// (its value cannot be changed).
    pub dim: bool,
}

/// The colour keys (see pixel.rs) a row is drawn in for one way it can be lit: its name's
/// colour, its value's (which the arrows share) and, for art that shows a level, the part not
/// reached.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) struct Ink {
    pub name: char,
    pub value: char,
    pub off: char,
}

impl Ink {
    /// Unlit, the name in the label colour and the value in the dim one; lit, the name cream
    /// and the value the frame's bright colour; dim, all of it the band colour, or the dim one
    /// when lit.
    pub fn of(lit: bool, dim: bool) -> Ink {
        let (name, value, off) = match (dim, lit) {
            (false, false) => ('L', 'd', 'p'),
            (false, true) => ('c', 'H', 'w'),
            (true, false) => ('b', 'b', 'p'),
            (true, true) => ('d', 'd', 'w'),
        };
        Ink { name, value, off }
    }
}

/// The colour `key` stands for in `pal`.
pub(super) fn colour(pal: &Palette, key: char) -> [u8; 3] {
    pal.key(key).expect("a row is drawn in the palette's keys")
}

/// A box on a row's line after its name: how wide it is, and the room kept before and after
/// it (CSS margins, which may be negative).
#[derive(Copy, Clone, Debug)]
pub(super) struct Item {
    pub w: u32,
    pub before: i32,
    pub after: i32,
}

/// The boxes after the name across a row `w` wide, as its flex line lays them out: the name
/// takes what room is left after them and the gap it keeps (none when there is none, and then
/// they run on past the row's end), and they stand after it in order. How wide the name's box
/// is, and each box's x.
pub(super) fn line(w: u32, items: &[Item]) -> (u32, Vec<i32>) {
    let inside = w as i32 - 2 * (BORDER + PAD) as i32;
    let taken: i32 = items.iter().map(|i| i.before + i.w as i32 + i.after).sum();
    let name = (inside - taken - NAME_GAP as i32).max(0);
    let mut x = (BORDER + PAD) as i32 + name + NAME_GAP as i32;
    let xs = items
        .iter()
        .map(|item| {
            let at = x + item.before;
            x = at + item.w as i32 + item.after;
            at
        })
        .collect();
    (name as u32, xs)
}

/// A row `w` wide before its words and art: clear, or the band when it is lit.
pub(super) fn base(w: u32, lit: bool, pal: &Palette) -> UndoFace {
    let mut face = blank(w, ROW_H);
    if lit {
        let rect = [0, 0, w as i32, ROW_H as i32];
        nine_slice(&mut face, &mirror(&BAND), rect, [1; 4], [1; 4], pal);
    }
    face
}

/// The name, in `colour` from the row's left, cut off `room` pixels in, as `overflow: hidden`
/// cuts it, and at the line's top, so a capital's accent loses the row it rises over the line.
pub(super) fn name(face: &mut UndoFace, text: &str, room: u32, colour: [u8; 3]) {
    let rows: Vec<String> = letters(text, ADVANCE)
        .into_iter()
        .map(|row| row.chars().take(room as usize).collect())
        .collect();
    let x = (BORDER + PAD) as i32;
    ink(face, &rows, x, BORDER as i32, colour);
}

/// A word on the row's line, its box's left at `x`. Nothing cuts it, so a capital's accent
/// rises over the line onto the band's frame line, as the browser draws it.
pub(super) fn word(face: &mut UndoFace, text: &str, x: i32, colour: [u8; 3]) {
    let rows = letters_over(text, ADVANCE, BORDER as usize);
    ink(face, &rows, x, 0, colour);
}

/// Art on the capitals' rows, its left at `x`.
pub(super) fn art(face: &mut UndoFace, rows: &[impl AsRef<str>], x: i32, pal: &Palette) {
    paint(face, rows, x, (BORDER + ART_TOP) as i32, pal);
}

/// `rows` with every '#' drawn in the colour key `key`.
pub(super) fn inked(rows: &[&str], key: char) -> Vec<String> {
    rows.iter()
        .map(|row| row.replace('#', &key.to_string()))
        .collect()
}

/// A row `width_u` art pixels wide and `ROW_H` tall, drawn as omega-row draws `row`.
pub fn row_face(width_u: u32, row: &Row, pal: &Palette) -> UndoFace {
    let ink = Ink::of(row.lit, row.dim);
    let value = row.value.filter(|v| !v.is_empty());
    let arrow = |before, after| Item {
        w: ARROW_W,
        before,
        after,
    };

    // left to right after the name: the ◀, the value, the ▶, the ›. Every letter's last pixel
    // is blank, and the value hands that pixel back on its right, so its ink ends as far from
    // what follows (or the band's end) as the name's starts from the band's.
    let mut items = Vec::new();
    let caret_room = row.carets || row.caret_room;
    if caret_room {
        items.push(arrow(0, ARROW_GAP));
    }
    if let Some(value) = value {
        items.push(Item {
            w: text_width(value),
            before: 0,
            after: -1,
        });
    }
    if caret_room {
        items.push(arrow(ARROW_GAP, 0));
    }
    if row.opens {
        items.push(arrow(ARROW_GAP, 0));
    }
    let (room, xs) = line(width_u, &items);
    let mut xs = xs.into_iter();

    let mut face = base(width_u, row.lit, pal);
    let label = row.label.to_uppercase();
    name(&mut face, &label, room, colour(pal, ink.name));
    let caret = inked(&CARET, ink.value);
    let carets_shown = row.carets && row.lit && !row.dim;
    if caret_room {
        let x = xs.next().expect("the ◀'s place");
        if carets_shown {
            art(&mut face, &caret, x, pal);
        }
    }
    if let Some(value) = value {
        let x = xs.next().expect("the value's place");
        word(&mut face, value, x, colour(pal, ink.value));
    }
    if caret_room {
        let x = xs.next().expect("the ▶'s place");
        if carets_shown {
            art(&mut face, &flip(&caret), x, pal);
        }
    }
    if row.opens {
        let x = xs.next().expect("the ›'s place");
        art(&mut face, &inked(&OPENS, ink.value), x, pal);
    }
    face
}

/// How wide a row is in a panel's well `well_w` wide: a pixel more each side.
pub fn row_width(well_w: u32) -> u32 {
    well_w + 2 * REACH
}

/// Where row `i` of a panel goes, `[x, y, w, h]` in the panel face's art pixels, from its well
/// (`PanelFace::well`): the rows stand one under the other from the well's top, each a pixel
/// into the well's sides.
pub fn row_rect(well: [u32; 4], i: u32) -> [u32; 4] {
    let [x, y, w, _] = well;
    [x - REACH, y + i * ROW_H, row_width(w), ROW_H]
}

/// How tall a panel is that is as tall as what its well holds, `content_h` art pixels of rows
/// and lines, as flow.css's panels that hug their rows are: above the well's inside, the
/// panel's stripe, its header, a frame line, the well's shadow line and its 2 pixels of room;
/// below it the same less the header.
pub fn panel_height(content_h: u32) -> u32 {
    let around = 1 + 1 + 1 + 2;
    content_h + HEADER + 2 * around
}
