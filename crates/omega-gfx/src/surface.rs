use std::ffi::c_void;
use std::fmt;

use crate::canvas::{out_h, out_w};

#[derive(Debug)]
pub enum GfxError {
    Context(String),
    Shader(String),
    Framebuffer(u32),
}

impl fmt::Display for GfxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GfxError::Context(m) => write!(f, "gl context: {m}"),
            GfxError::Shader(m) => write!(f, "shader: {m}"),
            GfxError::Framebuffer(s) => write!(f, "framebuffer incomplete: 0x{s:x}"),
        }
    }
}

impl std::error::Error for GfxError {}

pub trait Surface {
    fn make_current(&mut self) -> Result<(), GfxError>;
    fn window_size(&self) -> (u32, u32);
    fn swap(&mut self) -> Result<(), GfxError>;
    fn proc_address(&self, name: &str) -> *const c_void;
    /// A context for the emulator's thread, OpenGL ES 3, for a core that draws on the GPU:
    /// sharing this one's objects where it can. None where there is no such context to be had.
    fn gpu_thread(&self) -> Result<Box<dyn crate::gpu::GpuThread>, GfxError> {
        Err(GfxError::Context(
            "no context for a core that draws on the GPU".into(),
        ))
    }
}

pub fn fit_scale(win_w: u32, win_h: u32) -> u32 {
    (win_w / out_w()).min(win_h / out_h()).max(1)
}

pub fn fit_rect(win_w: u32, win_h: u32) -> (i32, i32, i32, i32) {
    let s = fit_scale(win_w, win_h) as i32;
    let (w, h) = (out_w() as i32 * s, out_h() as i32 * s);
    ((win_w as i32 - w) / 2, (win_h as i32 - h) / 2, w, h)
}

pub fn blit_rect_fit(panel: (u32, u32), shake: f32) -> (i32, i32, i32, i32) {
    let scale = (panel.0 as f32 / out_w() as f32).min(panel.1 as f32 / out_h() as f32);
    let w = (out_w() as f32 * scale).round() as i32;
    let h = (out_h() as f32 * scale).round() as i32;
    let dx = (shake * scale).round() as i32;
    (
        (panel.0 as i32 - w) / 2 + dx,
        (panel.1 as i32 - h) / 2,
        w,
        h,
    )
}

pub fn blit_rect(window: (u32, u32), shake: f32) -> (i32, i32, i32, i32) {
    if window.0 < out_w() || window.1 < out_h() {
        return blit_rect_fit(window, shake);
    }
    let (x, y, w, h) = fit_rect(window.0, window.1);
    let dx = shake * fit_scale(window.0, window.1) as f32;
    (x + dx.round() as i32, y, w, h)
}

/// How far the picture is turned counter-clockwise on its way to the panel, for a panel
/// mounted on its side.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum Rotation {
    #[default]
    R0,
    R90,
    R180,
    R270,
}

impl Rotation {
    /// The turn for `degrees` counter-clockwise, to the nearest quarter.
    pub fn from_ccw_degrees(degrees: u32) -> Rotation {
        match ((degrees + 45) / 90) % 4 {
            1 => Rotation::R90,
            2 => Rotation::R180,
            3 => Rotation::R270,
            _ => Rotation::R0,
        }
    }

    pub fn quarter(self) -> bool {
        matches!(self, Rotation::R90 | Rotation::R270)
    }

    /// The panel as the player holds it: a quarter turn swaps its sides.
    pub fn upright(self, panel: (u32, u32)) -> (u32, u32) {
        match self.quarter() {
            true => (panel.1, panel.0),
            false => panel,
        }
    }

    /// Takes a point of the viewport, centred on 0 and running -0.5 to 0.5 with y up, to the
    /// same point of the upright picture: the columns of the turned blit's matrix.
    pub fn uv_turn(self) -> [f32; 4] {
        match self {
            Rotation::R0 => [1.0, 0.0, 0.0, 1.0],
            Rotation::R90 => [0.0, -1.0, 1.0, 0.0],
            Rotation::R180 => [-1.0, 0.0, 0.0, -1.0],
            Rotation::R270 => [0.0, 1.0, -1.0, 0.0],
        }
    }
}

/// Where the composite goes on a `window` the panel's own way round, turned by `rot`: the
/// fit is worked out on the upright picture, then turned onto the panel. Viewport pixels,
/// from the bottom left.
pub fn panel_rect(window: (u32, u32), rot: Rotation, shake: f32) -> (i32, i32, i32, i32) {
    let upright = rot.upright(window);
    let (x, y, w, h) = blit_rect(upright, shake);
    let (uw, uh) = (upright.0 as i32, upright.1 as i32);
    match rot {
        Rotation::R0 => (x, y, w, h),
        Rotation::R90 => (uh - y - h, x, h, w),
        Rotation::R180 => (uw - x - w, uh - y - h, w, h),
        Rotation::R270 => (y, uw - x - w, h, w),
    }
}
