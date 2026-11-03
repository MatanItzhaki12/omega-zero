#![cfg(target_os = "linux")]

//! A core that draws on the GPU, as the emulator's thread runs it: an OpenGL ES 3 context of its
//! own, the framebuffers it draws into, and the frames handed back the right way up.

use omega_gfx::{Compositor, GpuFrame, GpuFrames, HeadlessSurface, Surface};
use std::sync::{Mutex, PoisonError};

static GL: Mutex<()> = Mutex::new(());

/// Draws as a core would: red over the whole of `size`, and green over its bottom `green` rows,
/// GL's bottom being the picture's.
fn draw(frames: &GpuFrames, size: (u32, u32), green: u32) {
    unsafe {
        gl::BindFramebuffer(gl::FRAMEBUFFER, frames.framebuffer());
        gl::Viewport(0, 0, size.0 as i32, size.1 as i32);
        gl::Disable(gl::SCISSOR_TEST);
        gl::ClearColor(1.0, 0.0, 0.0, 1.0);
        gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT | gl::STENCIL_BUFFER_BIT);
        gl::Enable(gl::SCISSOR_TEST);
        gl::Scissor(0, 0, size.0 as i32, green as i32);
        gl::ClearColor(0.0, 1.0, 0.0, 1.0);
        gl::Clear(gl::COLOR_BUFFER_BIT);
        gl::Disable(gl::SCISSOR_TEST);
        gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
    }
}

#[test]
fn a_frame_drawn_on_the_emulators_thread_comes_back_the_right_way_up() {
    let _g = GL.lock().unwrap_or_else(PoisonError::into_inner);
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    // the compositor loads the GL functions both contexts call
    let Ok(_compositor) = Compositor::new(&surface) else {
        return;
    };
    let gpu = surface.gpu_thread().expect("an OpenGL ES 3 context");
    assert!(!gpu.shares(), "desktop GL and ES share nothing");
    let frames = std::thread::spawn(move || {
        gpu.make_current()
            .expect("current on the emulator's thread");
        assert!(!gpu.proc_address("glClear").is_null());
        let mut frames = GpuFrames::new((64, 32), gpu.shares()).expect("the framebuffers");
        // before the core draws anything, its framebuffer is black, not what the GPU's memory held
        let fresh = frames.finish((8, 8));
        let first = frames.framebuffer();
        draw(&frames, (48, 20), 5);
        let a = frames.finish((48, 20));
        assert_eq!(
            frames.framebuffer(),
            first,
            "the core is told of another framebuffer: one that caches it would draw into the wrong one"
        );
        // a frame the core skips drawing hands over the last picture it drew again
        let skipped = frames.finish((48, 20));
        draw(&frames, (64, 32), 0);
        let b = frames.finish((64, 32));
        (a, b, skipped, fresh)
    })
    .join()
    .expect("the emulator's thread");
    let GpuFrame::Pixels(px, size) = frames.0 else {
        panic!("a context that shares nothing handed over a texture");
    };
    assert_eq!(size, (48, 20));
    let at = |x: usize, y: usize| {
        let o = (y * 48 + x) * 4;
        [px[o + 2], px[o + 1], px[o]]
    };
    assert_eq!(at(10, 0), [255, 0, 0], "the top row");
    assert_eq!(at(10, 14), [255, 0, 0]);
    // the bottom five rows, GL's first, are green at the bottom of the picture
    assert_eq!(at(10, 15), [0, 255, 0], "the picture came back upside down");
    assert_eq!(at(47, 19), [0, 255, 0]);
    assert_eq!(frames.1.size(), (64, 32));
    let GpuFrame::Pixels(again, _) = frames.2 else {
        panic!("a texture handed over");
    };
    assert_eq!(again, px, "a skipped frame handed over something else");
    let GpuFrame::Pixels(black, _) = frames.3 else {
        panic!("a texture handed over");
    };
    assert!(
        black.chunks_exact(4).all(|p| p[..3] == [0, 0, 0]),
        "a fresh framebuffer showed what the GPU's memory held"
    );
}
