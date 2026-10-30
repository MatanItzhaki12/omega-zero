use std::path::Path;

use crate::core::{AvInfo, ButtonMask, CoreError, RetroCore, GBA_H, GBA_W};

const FPS: f64 = 59.7275;
const SAMPLE_RATE: f64 = 32768.0;
const TONE_HZ: f64 = 440.0;
const AMPLITUDE: f64 = 6000.0;
const SRAM_LEN: usize = 8 * 1024;

/// A core with no game in it: a moving test pattern and a steady tone, a GBA's screen at a GBA's
/// rate unless it is built otherwise. Its state is the count of frames it has run.
pub struct MockCore {
    frame: u64,
    size: (u32, u32),
    /// From this frame on the picture is this size instead, as a PlayStation game changes
    /// resolution between its menus and its game.
    resize: Option<(u64, (u32, u32))>,
    fps: f64,
    video: Vec<u8>,
    drawn: (u32, u32),
    audio: Vec<i16>,
    sram: Option<Vec<u8>>,
    skip_next: bool,
    discs: usize,
    disc: usize,
}

impl Default for MockCore {
    fn default() -> Self {
        Self::new()
    }
}

impl MockCore {
    pub fn new() -> Self {
        Self::sized(GBA_W, GBA_H)
    }

    /// A mock that draws `w` by `h` pictures, such as a SNES game's 256x224.
    pub fn sized(w: u32, h: u32) -> Self {
        let mut c = MockCore {
            frame: 0,
            size: (w, h),
            resize: None,
            fps: FPS,
            video: Vec::new(),
            drawn: (0, 0),
            audio: Vec::new(),
            sram: None,
            skip_next: false,
            discs: 1,
            disc: 0,
        };
        c.render();
        c
    }

    /// The same mock, its pictures `w` by `h` from frame `frame` on.
    pub fn resized_at(mut self, frame: u64, w: u32, h: u32) -> Self {
        self.resize = Some((frame, (w, h)));
        self.render();
        self
    }

    /// The same mock running at `fps` frames a second, such as a PAL game's 50.
    pub fn at_fps(mut self, fps: f64) -> Self {
        self.fps = fps;
        self
    }

    /// The same mock with a game on `discs` discs, the first in the drive.
    pub fn with_discs(mut self, discs: usize) -> Self {
        self.discs = discs.max(1);
        self
    }

    fn samples_at(&self, frame: u64) -> u64 {
        (frame as f64 * SAMPLE_RATE / self.fps) as u64
    }

    fn size_now(&self) -> (u32, u32) {
        match self.resize {
            Some((from, size)) if self.frame >= from => size,
            _ => self.size,
        }
    }

    fn render(&mut self) {
        let (w, h) = self.size_now();
        self.drawn = (w, h);
        self.video.resize((w * h * 4) as usize, 0);
        let t = self.frame as u32;
        let mut i = 0;
        for y in 0..h {
            let g = (y * 255 / h.saturating_sub(1).max(1)) as u8;
            for x in 0..w {
                let r = x.wrapping_add(t) as u8;
                let b = if (x.wrapping_add(y).wrapping_add(t) / 8) % 2 == 0 {
                    0
                } else {
                    255
                };
                self.video[i] = b;
                self.video[i + 1] = g;
                self.video[i + 2] = r;
                self.video[i + 3] = 0;
                i += 4;
            }
        }
    }
}

impl RetroCore for MockCore {
    fn load(&mut self, _rom: &Path) -> Result<(), CoreError> {
        self.frame = 0;
        self.audio.clear();
        self.sram = Some(vec![0; SRAM_LEN]);
        self.render();
        Ok(())
    }

    fn run_frame(&mut self, _input: ButtonMask) {
        let start = self.samples_at(self.frame);
        self.frame += 1;
        let end = self.samples_at(self.frame);
        self.audio.reserve(((end - start) * 2) as usize);
        for n in start..end {
            let v = (std::f64::consts::TAU * TONE_HZ * n as f64 / SAMPLE_RATE).sin() * AMPLITUDE;
            let s = v as i16;
            self.audio.push(s);
            self.audio.push(s);
        }
        if !self.skip_next {
            self.render();
        }
    }

    fn set_frame_skip(&mut self, skip: bool) {
        self.skip_next = skip;
    }

    fn video_xrgb8888(&self) -> &[u8] {
        &self.video
    }

    fn video_size(&self) -> (u32, u32) {
        self.drawn
    }

    fn take_audio(&mut self) -> Vec<i16> {
        std::mem::take(&mut self.audio)
    }

    fn serialize(&mut self) -> Result<Vec<u8>, CoreError> {
        Ok(self.frame.to_le_bytes().to_vec())
    }

    fn reset(&mut self) {
        self.frame = 0;
        self.audio.clear();
        self.render();
    }

    fn unserialize(&mut self, data: &[u8]) -> Result<(), CoreError> {
        let bytes: [u8; 8] = data
            .try_into()
            .map_err(|_| CoreError::State(format!("expected 8 bytes, got {}", data.len())))?;
        self.frame = u64::from_le_bytes(bytes);
        self.audio.clear();
        self.render();
        Ok(())
    }

    fn save_ram(&self) -> Option<Vec<u8>> {
        self.sram.clone()
    }

    fn load_save_ram(&mut self, data: &[u8]) -> Result<(), CoreError> {
        match &mut self.sram {
            Some(s) if s.len() == data.len() => {
                s.copy_from_slice(data);
                Ok(())
            }
            Some(s) => Err(CoreError::State(format!(
                "save ram is {} bytes, got {}",
                s.len(),
                data.len()
            ))),
            None => Err(CoreError::State("no rom loaded".into())),
        }
    }

    fn av_info(&self) -> AvInfo {
        AvInfo {
            fps: self.fps,
            sample_rate: SAMPLE_RATE,
        }
    }

    fn disc_count(&self) -> usize {
        self.discs
    }

    fn disc_index(&self) -> usize {
        self.disc
    }

    fn set_disc(&mut self, index: usize) -> bool {
        if index >= self.discs {
            return false;
        }
        self.disc = index;
        true
    }

    fn disc_path(&self, index: usize) -> Option<String> {
        (index < self.discs).then(|| format!("mock disc {}", index + 1))
    }

    fn set_initial_disc(&mut self, index: usize, path: &str) {
        if self.disc_path(index).as_deref() == Some(path) {
            self.disc = index;
        }
    }
}
