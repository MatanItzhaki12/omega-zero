#![cfg(target_os = "linux")]

//! A core that draws on the GPU, run by the emulator's thread as PPSSPP would be: offered the
//! GPU, its context made current and reset once its game is loaded, its frames drawn into the
//! framebuffer it is handed, and published the right way up. A test of its own: the context for
//! such cores is given to the emulator once a process.

use std::ffi::{c_void, CString};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use omega_gfx::{Compositor, HeadlessSurface, Surface};
use omega_retro::{AvInfo, ButtonMask, CoreError, GlHost, GpuRequest, RetroCore};
use omega_zero::audio::{AudioSink, StubSink};
use omega_zero::emu::{CoreState, EmuHandle, Speed};

type BindFramebuffer = unsafe extern "C" fn(u32, u32);
type Viewport = unsafe extern "C" fn(i32, i32, i32, i32);
type ClearColor = unsafe extern "C" fn(f32, f32, f32, f32);
type Clear = unsafe extern "C" fn(u32);
type Scissor = unsafe extern "C" fn(i32, i32, i32, i32);
type Toggle = unsafe extern "C" fn(u32);

const FRAMEBUFFER: u32 = 0x8D40;
const COLOR_BUFFER_BIT: u32 = 0x4000;
const SCISSOR_TEST: u32 = 0x0C11;

/// The GL functions the test core draws with, as a core looks them up.
struct Gl {
    bind_framebuffer: BindFramebuffer,
    viewport: Viewport,
    clear_color: ClearColor,
    clear: Clear,
    scissor: Scissor,
    enable: Toggle,
    disable: Toggle,
}

impl Gl {
    fn load(host: &dyn GlHost) -> Gl {
        let f = |name: &str| {
            let at = host.proc_address(&CString::new(name).unwrap());
            assert!(!at.is_null(), "no {name}");
            at
        };
        unsafe {
            Gl {
                bind_framebuffer: std::mem::transmute::<*const c_void, BindFramebuffer>(f(
                    "glBindFramebuffer",
                )),
                viewport: std::mem::transmute::<*const c_void, Viewport>(f("glViewport")),
                clear_color: std::mem::transmute::<*const c_void, ClearColor>(f("glClearColor")),
                clear: std::mem::transmute::<*const c_void, Clear>(f("glClear")),
                scissor: std::mem::transmute::<*const c_void, Scissor>(f("glScissor")),
                enable: std::mem::transmute::<*const c_void, Toggle>(f("glEnable")),
                disable: std::mem::transmute::<*const c_void, Toggle>(f("glDisable")),
            }
        }
    }
}

/// A core that draws each frame on the GPU: red, its bottom quarter green.
#[derive(Default)]
struct GpuDrawer {
    host: Option<Box<dyn GlHost>>,
    gl: Option<Gl>,
    drawn: Option<(u32, u32)>,
    /// What happened to it as it was put away, in order.
    ended: Arc<Mutex<Vec<&'static str>>>,
}

impl Drop for GpuDrawer {
    fn drop(&mut self) {
        self.ended.lock().unwrap().push("dropped");
    }
}

// SAFETY: the function pointers are GL's, called on the emulator's thread alone
unsafe impl Send for GpuDrawer {}

const SIZE: (u32, u32) = (96, 48);

impl RetroCore for GpuDrawer {
    fn load(&mut self, _rom: &Path) -> Result<(), CoreError> {
        Ok(())
    }
    fn offer_gpu(&mut self, host: Box<dyn GlHost>) {
        self.host = Some(host);
    }
    fn gpu_request(&self) -> Option<GpuRequest> {
        self.host.as_ref().map(|_| GpuRequest {
            version: (3, 0),
            depth: true,
            stencil: true,
            bottom_left: true,
        })
    }
    fn gpu_context_reset(&mut self) {
        self.gl = self.host.as_deref().map(Gl::load);
    }
    fn gpu_context_destroy(&mut self) {
        self.ended.lock().unwrap().push("context destroyed");
    }
    fn gpu_frame(&self) -> Option<(u32, u32)> {
        self.drawn
    }
    fn run_frame(&mut self, _input: ButtonMask) {
        let (Some(host), Some(gl)) = (&self.host, &self.gl) else {
            self.drawn = None;
            return;
        };
        let (w, h) = (SIZE.0 as i32, SIZE.1 as i32);
        unsafe {
            (gl.bind_framebuffer)(FRAMEBUFFER, host.framebuffer() as u32);
            (gl.viewport)(0, 0, w, h);
            (gl.disable)(SCISSOR_TEST);
            (gl.clear_color)(1.0, 0.0, 0.0, 1.0);
            (gl.clear)(COLOR_BUFFER_BIT);
            (gl.enable)(SCISSOR_TEST);
            (gl.scissor)(0, 0, w, h / 4);
            (gl.clear_color)(0.0, 1.0, 0.0, 1.0);
            (gl.clear)(COLOR_BUFFER_BIT);
            (gl.disable)(SCISSOR_TEST);
            (gl.bind_framebuffer)(FRAMEBUFFER, 0);
        }
        self.drawn = Some(SIZE);
    }
    fn video_xrgb8888(&self) -> &[u8] {
        &[]
    }
    fn take_audio(&mut self) -> Vec<i16> {
        Vec::new()
    }
    fn serialize(&mut self) -> Result<Vec<u8>, CoreError> {
        Ok(Vec::new())
    }
    fn unserialize(&mut self, _data: &[u8]) -> Result<(), CoreError> {
        Ok(())
    }
    fn save_ram(&self) -> Option<Vec<u8>> {
        None
    }
    fn load_save_ram(&mut self, _data: &[u8]) -> Result<(), CoreError> {
        Ok(())
    }
    fn av_info(&self) -> AvInfo {
        AvInfo {
            fps: 60.0,
            sample_rate: 48_000.0,
        }
    }
}

#[test]
fn a_core_that_draws_on_the_gpu_publishes_its_frames_the_right_way_up() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(_compositor) = Compositor::new(&surface) else {
        return;
    };
    omega_zero::gpu_core::provide(surface.gpu_thread().expect("an OpenGL ES 3 context"));
    let mut sink = StubSink::new();
    sink.open(32_768).expect("the stub sink");
    let drained = sink.clone();
    std::thread::spawn(move || loop {
        drained.device_drain();
        std::thread::sleep(Duration::from_millis(2));
    });
    let ended = Arc::new(Mutex::new(Vec::new()));
    let mut drawer = GpuDrawer::default();
    drawer.ended = ended.clone();
    let emu = EmuHandle::spawn(
        Box::new(drawer),
        PathBuf::from("gpu"),
        sink.ring(),
        None,
        None,
    );
    emu.set_speed(Speed::Normal);
    let deadline = Instant::now() + Duration::from_secs(10);
    let frame = loop {
        assert!(Instant::now() < deadline, "no frame was published");
        assert_ne!(emu.state(), CoreState::Failed, "the core failed to load");
        if let Some(f) = emu.latest_frame() {
            break f;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(frame.size(), SIZE);
    let at = |x: usize, y: usize| {
        let o = (y * SIZE.0 as usize + x) * 4;
        [frame[o + 2], frame[o + 1], frame[o]]
    };
    assert_eq!(at(10, 2), [255, 0, 0], "the top of the frame");
    assert_eq!(at(50, 47), [0, 255, 0], "the frame came back upside down");
    assert_eq!(at(95, 36), [0, 255, 0]);
    drop(emu);
    // put away as libretro asks: told its context is going, then unloaded; never unloaded
    // with no word, which left PPSSPP's next game on what the last one made
    let deadline = Instant::now() + Duration::from_secs(10);
    while ended.lock().unwrap().len() < 2 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(*ended.lock().unwrap(), ["context destroyed", "dropped"]);
}
