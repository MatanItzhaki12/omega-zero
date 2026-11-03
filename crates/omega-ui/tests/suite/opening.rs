//! The opening's lettering: Pixel Code on its grid, each pixel three screen pixels wide and
//! two tall, bold by a screen pixel, a row of mint at a time.

use omega_ui::{opening_line, UndoFace, OPENING_ADVANCE, OPENING_PITCH, OPENING_RISE};

fn at(face: &UndoFace, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * face.w + x) * 4) as usize;
    face.rgba[i..i + 4].try_into().unwrap()
}

/// The face's pixel `col` across and `row` down, from the row above the capitals, judged by
/// its middle screen pixel: its first can be the bold of the pixel to its left.
fn inked(face: &UndoFace, col: u32, row: u32) -> bool {
    at(face, col * 3 + 1, row * 2)[3] == 255
}

#[test]
fn a_line_is_a_cell_a_letter_and_the_face_tall() {
    assert_eq!((OPENING_ADVANCE, OPENING_PITCH, OPENING_RISE), (18, 18, 2));
    let face = opening_line("OMEGA ZERO MATAN HANDHELD");
    assert_eq!((face.w, face.h), (25 * 18 + 1, 20));
    assert_eq!(face.rgba.len(), (face.w * face.h * 4) as usize);
    let blank = opening_line("   ");
    assert!(blank.rgba.iter().all(|&b| b == 0), "a space inks nothing");
    assert_eq!(opening_line("").w, 1);
}

#[test]
fn a_capital_is_pixel_codes_own_on_the_grid() {
    let face = opening_line("M");
    let drawn: Vec<String> = (0..10)
        .map(|row| {
            (0..6)
                .map(|col| if inked(&face, col, row) { '#' } else { '.' })
                .collect()
        })
        .collect();
    assert_eq!(
        drawn,
        [
            "......", "#...#.", "##.##.", "#.#.#.", "#...#.", "#...#.", "#...#.", "#...#.",
            "......", "......",
        ],
        "a row above the capitals, seven of them, two of tail"
    );
}

#[test]
fn a_tail_hangs_below_the_capitals() {
    let face = opening_line("g");
    let rows: Vec<bool> = (0..10)
        .map(|r| (0..6).any(|c| inked(&face, c, r)))
        .collect();
    assert!(rows[8], "g's tail hangs under the line: {rows:?}");
    assert!(
        !rows[0] && !rows[1] && !rows[2],
        "and it stands no taller than x: {rows:?}"
    );
}

#[test]
fn every_face_pixel_is_three_wide_two_tall_and_a_screen_pixel_bolder() {
    let face = opening_line("M");
    // M's stems alone, on its first row and its last four: the left one three screen pixels
    // and the bold's fourth, the right one on the cell's fifth column, bold into the sixth's
    for y in (2..4).chain(8..16) {
        assert!((0..4).all(|x| at(&face, x, y)[3] == 255), "row {y}");
        assert!((4..12).all(|x| at(&face, x, y)[3] == 0), "row {y}");
        assert!((12..16).all(|x| at(&face, x, y)[3] == 255), "row {y}");
        assert!((16..face.w).all(|x| at(&face, x, y)[3] == 0), "row {y}");
    }
    assert!((0..face.w).all(|x| at(&face, x, 0)[3] == 0 && at(&face, x, 1)[3] == 0));
}

#[test]
fn the_rows_are_x5s_mint_pale_through_the_middle() {
    let face = opening_line("M");
    let mint = [161, 228, 208, 255];
    let pale = [214, 238, 232, 255];
    assert_eq!(at(&face, 0, 2), mint, "the capitals' first row");
    assert_eq!(at(&face, 0, 5), mint);
    assert_eq!(at(&face, 0, 6), pale, "their third and fourth");
    assert_eq!(at(&face, 0, 9), pale);
    assert_eq!(at(&face, 0, 10), [176, 230, 214, 255]);
    assert_eq!(at(&face, 0, 14), [140, 204, 186, 255], "their foot, deeper");
    let tail = opening_line("g");
    let foot: Vec<[u8; 4]> = (0..tail.w)
        .map(|x| at(&tail, x, 16))
        .filter(|p| p[3] == 255)
        .collect();
    assert!(!foot.is_empty());
    assert!(
        foot.iter().all(|p| *p == [116, 178, 162, 255]),
        "the tails deeper still"
    );
}

#[test]
fn a_masked_codes_stars_sit_in_the_middle_of_the_line() {
    let face = opening_line("****");
    let rows: Vec<bool> = (0..10)
        .map(|r| (0..24).any(|c| inked(&face, c, r)))
        .collect();
    assert!(rows.iter().any(|&r| r));
    assert!(
        !rows[0] && !rows[1] && !rows[2],
        "not up by the capitals' tops, as Pixel Code sets them: {rows:?}"
    );
    assert!(!rows[8] && !rows[9], "nor down in the tails: {rows:?}");
}
