//! The star a favorite wears on the shelf, at its top right corner, as pixel art: a gold star
//! with a dark outline and a lighter point, so it reads over any label or backdrop.

use crate::UndoFace;

const STAR: [&str; 11] = [
    ".....#.....",
    "....#h#....",
    "....#h#....",
    "#####hy####",
    "#hhhyyyyyy#",
    ".#yyyyyyy#.",
    "..#yyyyy#..",
    "..#yy#yy#..",
    ".#yy#.#yy#.",
    ".#y#...#y#.",
    ".##.....##.",
];

const OUTLINE: [u8; 4] = [0x2a, 0x1e, 0x10, 0xff];
const GOLD: [u8; 4] = [0xf4, 0xc2, 0x30, 0xff];
const LIGHT: [u8; 4] = [0xff, 0xec, 0x9a, 0xff];

/// The star's art pixels, a texel each.
pub fn star_face() -> UndoFace {
    let (w, h) = (STAR[0].len() as u32, STAR.len() as u32);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for (y, row) in STAR.iter().enumerate() {
        for (x, c) in row.bytes().enumerate() {
            let px = match c {
                b'#' => OUTLINE,
                b'y' => GOLD,
                b'h' => LIGHT,
                _ => continue,
            };
            let at = (y * w as usize + x) * 4;
            rgba[at..at + 4].copy_from_slice(&px);
        }
    }
    UndoFace { rgba, w, h }
}

/// How big the star is drawn beside a cart at full size, in canvas pixels.
pub const STAR_PX: f32 = 22.0;
