use std::f32::consts::PI;

const BRIGHTEN_SCANLINES: f32 = 16.0;
const BRIGHTEN_LCD: f32 = 4.0;

/// How bright the grid's lines between game pixels are: the last panel pixel of each row and
/// column of a game pixel's 3x3, as Slot's Grid draws them.
const GRID_LINE: f32 = 0.8;

/// What a game at 3x is drawn through: a 3x3 cell multiplied over every game pixel.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum Mask {
    /// GigaHerz's LCD3x: scanlines and the panel's red, green and blue stripes.
    #[default]
    Lcd,
    /// A thin dark line between game pixels, and nothing else.
    Grid,
    /// The game's pixels as they are, each a plain 3x3 square.
    Off,
}

impl Mask {
    /// The cell, by row, column and channel.
    pub fn cell(self) -> [[[f32; 3]; 3]; 3] {
        match self {
            Mask::Lcd => lcd3x_mask(),
            Mask::Grid => grid_mask(),
            Mask::Off => [[[1.0; 3]; 3]; 3],
        }
    }

    /// The cell as the 3x3 RGBA8 texture the game pass samples.
    pub fn texture_rgba8(self) -> [u8; 3 * 3 * 4] {
        texture_of(&self.cell())
    }
}

pub fn lcd3x_mask() -> [[[f32; 3]; 3]; 3] {
    let mut mask = [[[0.0f32; 3]; 3]; 3];
    for (oy, row) in mask.iter_mut().enumerate() {
        let yfactor = (BRIGHTEN_SCANLINES + (PI * (oy as f32 + 0.5) * 2.0 / 3.0).sin())
            / (BRIGHTEN_SCANLINES + 1.0);
        for (ox, cell) in row.iter_mut().enumerate() {
            for (c, v) in cell.iter_mut().enumerate() {
                let xfactor = (BRIGHTEN_LCD
                    + (PI * (ox as f32 + 0.5) * 2.0 / 3.0 + c as f32 * 2.0 * PI / 3.0).sin())
                    / (BRIGHTEN_LCD + 1.0);
                *v = yfactor * xfactor;
            }
        }
    }
    mask
}

fn grid_mask() -> [[[f32; 3]; 3]; 3] {
    let line = |at: usize| if at == 2 { GRID_LINE } else { 1.0 };
    let mut mask = [[[0.0f32; 3]; 3]; 3];
    for (oy, row) in mask.iter_mut().enumerate() {
        for (ox, cell) in row.iter_mut().enumerate() {
            *cell = [line(oy) * line(ox); 3];
        }
    }
    mask
}

pub fn mask_texture_rgba8() -> [u8; 3 * 3 * 4] {
    texture_of(&lcd3x_mask())
}

fn texture_of(mask: &[[[f32; 3]; 3]; 3]) -> [u8; 3 * 3 * 4] {
    let mut tex = [255u8; 3 * 3 * 4];
    for (texel, cell) in tex.chunks_exact_mut(4).zip(mask.iter().flatten()) {
        for (out, v) in texel.iter_mut().zip(cell) {
            *out = (v * 255.0).round() as u8;
        }
    }
    tex
}
