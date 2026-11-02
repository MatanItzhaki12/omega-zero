//! The headless surface on Linux: an OpenGL 3.3 core context from Mesa's surfaceless EGL, with no
//! window and no display, drawn by its software renderer where there is no GPU, as on a CI
//! runner. omega-zero draws into its own framebuffer and reads its frames back from there, so
//! the context needs no surface of its own.

use crate::canvas::{out_h, out_w};
use crate::surface::{GfxError, Surface};
use libloading::Library;
use std::ffi::{c_void, CString};

type Display = *mut c_void;
type Config = *mut c_void;
type Context = *mut c_void;

const EGL_NONE: i32 = 0x3038;
const EGL_ALPHA_SIZE: i32 = 0x3021;
const EGL_BLUE_SIZE: i32 = 0x3022;
const EGL_GREEN_SIZE: i32 = 0x3023;
const EGL_RED_SIZE: i32 = 0x3024;
const EGL_SURFACE_TYPE: i32 = 0x3033;
const EGL_RENDERABLE_TYPE: i32 = 0x3040;
const EGL_PBUFFER_BIT: i32 = 0x0001;
const EGL_OPENGL_BIT: i32 = 0x0008;
const EGL_OPENGL_API: u32 = 0x30A2;
const EGL_CONTEXT_MAJOR_VERSION: i32 = 0x3098;
const EGL_CONTEXT_MINOR_VERSION: i32 = 0x30FB;
const EGL_CONTEXT_OPENGL_PROFILE_MASK: i32 = 0x30FD;
const EGL_CONTEXT_OPENGL_CORE_PROFILE_BIT: i32 = 0x0001;
/// Mesa's platform that draws with no window system at all.
const EGL_PLATFORM_SURFACELESS_MESA: u32 = 0x31DD;

type GetPlatformDisplay = unsafe extern "C" fn(u32, *mut c_void, *const isize) -> Display;
type Initialize = unsafe extern "C" fn(Display, *mut i32, *mut i32) -> u32;
type BindApi = unsafe extern "C" fn(u32) -> u32;
type ChooseConfig = unsafe extern "C" fn(Display, *const i32, *mut Config, i32, *mut i32) -> u32;
type CreateContext = unsafe extern "C" fn(Display, Config, Context, *const i32) -> Context;
type MakeCurrent = unsafe extern "C" fn(Display, *mut c_void, *mut c_void, Context) -> u32;
type GetProcAddress = unsafe extern "C" fn(*const std::ffi::c_char) -> *const c_void;
type GetError = unsafe extern "C" fn() -> i32;
type DestroyContext = unsafe extern "C" fn(Display, Context) -> u32;

pub struct HeadlessSurface {
    display: Display,
    context: Context,
    make_current: MakeCurrent,
    get_proc_address: GetProcAddress,
    get_error: GetError,
    destroy_context: DestroyContext,
}

/// libEGL, opened once and never closed: Mesa keeps threads and exit handlers of its own in
/// there, and a process that unloads it under them goes down on its way out.
fn egl() -> Result<&'static Library, GfxError> {
    static EGL: std::sync::OnceLock<Result<Library, String>> = std::sync::OnceLock::new();
    EGL.get_or_init(|| unsafe { Library::new("libEGL.so.1") }.map_err(|e| e.to_string()))
        .as_ref()
        .map_err(|e| GfxError::Context(format!("libEGL.so.1: {e}")))
}

unsafe fn sym<T: Copy>(lib: &Library, name: &str) -> Result<T, GfxError> {
    lib.get::<T>(name.as_bytes())
        .map(|s| *s)
        .map_err(|e| GfxError::Context(format!("{name}: {e}")))
}

impl HeadlessSurface {
    pub fn new() -> Result<Self, GfxError> {
        let lib = egl()?;
        unsafe {
            let get_platform_display: GetPlatformDisplay = sym(lib, "eglGetPlatformDisplay")?;
            let initialize: Initialize = sym(lib, "eglInitialize")?;
            let bind_api: BindApi = sym(lib, "eglBindAPI")?;
            let choose_config: ChooseConfig = sym(lib, "eglChooseConfig")?;
            let create_context: CreateContext = sym(lib, "eglCreateContext")?;
            let make_current: MakeCurrent = sym(lib, "eglMakeCurrent")?;
            let get_error: GetError = sym(lib, "eglGetError")?;
            let fail = |what: &str| crate::fbdev::egl_error(what, get_error());

            let display = get_platform_display(
                EGL_PLATFORM_SURFACELESS_MESA,
                std::ptr::null_mut(),
                std::ptr::null(),
            );
            if display.is_null() {
                return Err(fail("eglGetPlatformDisplay (surfaceless)"));
            }
            if initialize(display, std::ptr::null_mut(), std::ptr::null_mut()) == 0 {
                return Err(fail("eglInitialize"));
            }
            if bind_api(EGL_OPENGL_API) == 0 {
                return Err(fail("eglBindAPI"));
            }
            let attrs = [
                EGL_SURFACE_TYPE,
                EGL_PBUFFER_BIT,
                EGL_RENDERABLE_TYPE,
                EGL_OPENGL_BIT,
                EGL_RED_SIZE,
                8,
                EGL_GREEN_SIZE,
                8,
                EGL_BLUE_SIZE,
                8,
                EGL_ALPHA_SIZE,
                8,
                EGL_NONE,
            ];
            let mut config: Config = std::ptr::null_mut();
            let mut count = 0;
            if choose_config(display, attrs.as_ptr(), &mut config, 1, &mut count) == 0 {
                return Err(fail("eglChooseConfig"));
            }
            if count == 0 {
                return Err(GfxError::Context(
                    "eglChooseConfig: no opengl config with 8/8/8/8 colour".into(),
                ));
            }
            // the shaders' own #version 330 core
            let context_attrs = [
                EGL_CONTEXT_MAJOR_VERSION,
                3,
                EGL_CONTEXT_MINOR_VERSION,
                3,
                EGL_CONTEXT_OPENGL_PROFILE_MASK,
                EGL_CONTEXT_OPENGL_CORE_PROFILE_BIT,
                EGL_NONE,
            ];
            let context = create_context(
                display,
                config,
                std::ptr::null_mut(),
                context_attrs.as_ptr(),
            );
            if context.is_null() {
                return Err(fail("eglCreateContext"));
            }
            let none = std::ptr::null_mut();
            if make_current(display, none, none, context) == 0 {
                return Err(fail("eglMakeCurrent"));
            }
            Ok(HeadlessSurface {
                display,
                context,
                make_current,
                get_proc_address: sym(lib, "eglGetProcAddress")?,
                get_error,
                destroy_context: sym(lib, "eglDestroyContext")?,
            })
        }
    }
}

impl Surface for HeadlessSurface {
    fn make_current(&mut self) -> Result<(), GfxError> {
        let none = std::ptr::null_mut();
        match unsafe { (self.make_current)(self.display, none, none, self.context) } {
            0 => Err(crate::fbdev::egl_error("eglMakeCurrent", unsafe {
                (self.get_error)()
            })),
            _ => Ok(()),
        }
    }

    fn window_size(&self) -> (u32, u32) {
        (out_w(), out_h())
    }

    fn swap(&mut self) -> Result<(), GfxError> {
        unsafe { gl::Flush() };
        Ok(())
    }

    fn proc_address(&self, name: &str) -> *const c_void {
        let Ok(sym) = CString::new(name) else {
            return std::ptr::null();
        };
        unsafe { (self.get_proc_address)(sym.as_ptr()) }
    }
}

impl Drop for HeadlessSurface {
    fn drop(&mut self) {
        let none = std::ptr::null_mut();
        unsafe {
            (self.make_current)(self.display, none, none, std::ptr::null_mut());
            // the display is left up: the tests' other threads draw on it too
            (self.destroy_context)(self.display, self.context);
        }
    }
}
