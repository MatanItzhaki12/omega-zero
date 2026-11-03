//! Cores that draw on the GPU, as PPSSPP does: the context they draw in on the emulator's
//! thread, the framebuffers they draw each frame into, and the frames handed to the compositor.
//!
//! The emulator's thread has a context of its own, OpenGL ES 3. Where it shares its objects with
//! the compositor's, as on the handheld, a frame is the texture the core drew, handed over with
//! a fence for the compositor to wait on before it draws it. Where it cannot, as on a computer
//! whose own context is desktop OpenGL, the frame is read back into memory and uploaded as any
//! other core's picture is.
//!
//! The core draws into one framebuffer of its own, every frame: a core may skip drawing a frame,
//! or draw it late, and its framebuffer still holds the last picture it drew. As each frame is
//! handed over, that picture is copied on the GPU into one of three textures that take turns, so
//! the core draws the next frame while the compositor still shows the one before.

use std::ffi::c_void;

use crate::surface::GfxError;

/// A context for the emulator's thread, made by the surface the compositor draws on.
pub trait GpuThread: Send {
    /// Makes the context current on the calling thread.
    fn make_current(&self) -> Result<(), GfxError>;
    /// The address of GL function `name` for the core, or null.
    fn proc_address(&self, name: &str) -> *const c_void;
    /// Whether the compositor's context sees this one's textures: frames are handed over as
    /// textures, else read back.
    fn shares(&self) -> bool;
    /// Makes no context current on the calling thread, before it ends: the context is lent to
    /// the next game's thread.
    fn release(&self);
}

/// How many framebuffers take turns.
const RING: usize = 3;

/// A frame a core drew on the GPU, as the compositor takes it.
pub enum GpuFrame {
    /// The texture it is in, the fence that says the drawing is done, and its size. Drawn with
    /// GL's origin at the bottom left: upside down.
    Texture {
        tex: u32,
        fence: usize,
        size: (u32, u32),
    },
    /// Its pixels, XRGB8888 as a core in memory draws them, the right way up.
    Pixels(Vec<u8>, (u32, u32)),
}

impl GpuFrame {
    pub fn size(&self) -> (u32, u32) {
        match self {
            GpuFrame::Texture { size, .. } | GpuFrame::Pixels(_, size) => *size,
        }
    }
}

// SAFETY: the fence is a GLsync handle, waited on by the compositor's context, which shares it
unsafe impl Send for GpuFrame {}

struct Target {
    fbo: u32,
    tex: u32,
    /// Depth and stencil, for the core's framebuffer; none for a texture frames are copied to.
    depth: u32,
}

impl Target {
    /// A framebuffer `w` by `h` drawing into a texture, with depth and stencil when `depth`,
    /// cleared to black so nothing left in the GPU's memory ever shows.
    fn new(w: i32, h: i32, depth: bool) -> Result<Target, GfxError> {
        let (mut fbo, mut tex, mut rb) = (0, 0, 0);
        unsafe {
            gl::GenTextures(1, &mut tex);
            gl::BindTexture(gl::TEXTURE_2D, tex);
            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::RGBA8 as i32,
                w,
                h,
                0,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                std::ptr::null(),
            );
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as i32);
            gl::GenFramebuffers(1, &mut fbo);
            gl::BindFramebuffer(gl::FRAMEBUFFER, fbo);
            gl::FramebufferTexture2D(
                gl::FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                gl::TEXTURE_2D,
                tex,
                0,
            );
            if depth {
                gl::GenRenderbuffers(1, &mut rb);
                gl::BindRenderbuffer(gl::RENDERBUFFER, rb);
                gl::RenderbufferStorage(gl::RENDERBUFFER, gl::DEPTH24_STENCIL8, w, h);
                gl::FramebufferRenderbuffer(
                    gl::FRAMEBUFFER,
                    gl::DEPTH_STENCIL_ATTACHMENT,
                    gl::RENDERBUFFER,
                    rb,
                );
            }
            let status = gl::CheckFramebufferStatus(gl::FRAMEBUFFER);
            if status == gl::FRAMEBUFFER_COMPLETE {
                gl::Viewport(0, 0, w, h);
                gl::ClearColor(0.0, 0.0, 0.0, 1.0);
                gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT | gl::STENCIL_BUFFER_BIT);
            }
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
            let target = Target {
                fbo,
                tex,
                depth: rb,
            };
            if status != gl::FRAMEBUFFER_COMPLETE {
                target.delete();
                return Err(GfxError::Framebuffer(status));
            }
            Ok(target)
        }
    }

    fn delete(&self) {
        unsafe {
            gl::DeleteFramebuffers(1, &self.fbo);
            if self.depth != 0 {
                gl::DeleteRenderbuffers(1, &self.depth);
            }
            gl::DeleteTextures(1, &self.tex);
        }
    }
}

/// The framebuffers a core draws into, on the emulator's thread with its context current.
pub struct GpuFrames {
    /// The one the core draws every frame into.
    core: Target,
    /// The textures frames are copied to and handed over in, taking turns.
    targets: Vec<Target>,
    next: usize,
    size: (u32, u32),
    shares: bool,
}

impl GpuFrames {
    /// The core's framebuffer `size` big, with depth and stencil, the most a core says it draws,
    /// and the three textures its frames are handed over in.
    pub fn new(size: (u32, u32), shares: bool) -> Result<GpuFrames, GfxError> {
        let (w, h) = (size.0.max(1) as i32, size.1.max(1) as i32);
        let core = Target::new(w, h, true)?;
        let mut targets = Vec::with_capacity(RING);
        for _ in 0..RING {
            match Target::new(w, h, false) {
                Ok(t) => targets.push(t),
                Err(e) => {
                    core.delete();
                    targets.iter().for_each(Target::delete);
                    return Err(e);
                }
            }
        }
        Ok(GpuFrames {
            core,
            targets,
            next: 0,
            size: (w as u32, h as u32),
            shares,
        })
    }

    /// The framebuffer the core draws into: the same one every frame.
    pub fn framebuffer(&self) -> u32 {
        self.core.fbo
    }

    /// How big the framebuffers are.
    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// The pixels of the core's last picture, `size` of it, XRGB8888 the right way up.
    pub fn read_last(&self, size: (u32, u32)) -> Vec<u8> {
        let size = (size.0.min(self.size.0), size.1.min(self.size.1));
        read_back(self.core.fbo, size)
    }

    /// The frame the core just drew, `size` of it from the bottom left, handed over: copied on
    /// the GPU into the next of the textures that take turns.
    pub fn finish(&mut self, size: (u32, u32)) -> GpuFrame {
        let size = (size.0.min(self.size.0), size.1.min(self.size.1));
        if !self.shares {
            return GpuFrame::Pixels(read_back(self.core.fbo, size), size);
        }
        let target = &self.targets[self.next];
        self.next = (self.next + 1) % RING;
        let (w, h) = (size.0 as i32, size.1 as i32);
        unsafe {
            gl::BindFramebuffer(gl::READ_FRAMEBUFFER, self.core.fbo);
            gl::BindFramebuffer(gl::DRAW_FRAMEBUFFER, target.fbo);
            gl::BlitFramebuffer(0, 0, w, h, 0, 0, w, h, gl::COLOR_BUFFER_BIT, gl::NEAREST);
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
        }
        let fence = unsafe {
            let fence = gl::FenceSync(gl::SYNC_GPU_COMMANDS_COMPLETE, 0);
            gl::Flush();
            fence
        };
        GpuFrame::Texture {
            tex: target.tex,
            fence: fence as usize,
            size,
        }
    }
}

impl Drop for GpuFrames {
    fn drop(&mut self) {
        self.core.delete();
        self.targets.iter().for_each(Target::delete);
    }
}

/// The pixels of `size` from the bottom left of framebuffer `fbo`, as XRGB8888 the right way
/// up: GL reads them bottom row first, RGBA.
fn read_back(fbo: u32, size: (u32, u32)) -> Vec<u8> {
    let (w, h) = (size.0 as usize, size.1 as usize);
    let mut rgba = vec![0u8; w * h * 4];
    unsafe {
        gl::BindFramebuffer(gl::FRAMEBUFFER, fbo);
        gl::PixelStorei(gl::PACK_ALIGNMENT, 1);
        gl::ReadPixels(
            0,
            0,
            w as i32,
            h as i32,
            gl::RGBA,
            gl::UNSIGNED_BYTE,
            rgba.as_mut_ptr() as *mut c_void,
        );
        gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
    }
    let mut out = vec![0u8; w * h * 4];
    for y in 0..h {
        let from = &rgba[(h - 1 - y) * w * 4..(h - y) * w * 4];
        let to = &mut out[y * w * 4..(y + 1) * w * 4];
        for (px, o) in from.chunks_exact(4).zip(to.chunks_exact_mut(4)) {
            // XRGB8888 in memory, little-endian: blue, green, red, then the unused byte
            o[0] = px[2];
            o[1] = px[1];
            o[2] = px[0];
            o[3] = 0;
        }
    }
    out
}

/// Lets a fence go unwaited, for a frame the compositor never took.
pub fn delete_fence(fence: usize) {
    if fence != 0 {
        unsafe { gl::DeleteSync(fence as gl::types::GLsync) };
    }
}

/// Waits, on the compositor's context, for the drawing a fence follows to be done, and lets
/// the fence go.
pub fn wait_and_free(fence: usize) {
    if fence == 0 {
        return;
    }
    let sync = fence as gl::types::GLsync;
    unsafe {
        gl::WaitSync(sync, 0, gl::TIMEOUT_IGNORED);
        gl::DeleteSync(sync);
    }
}
