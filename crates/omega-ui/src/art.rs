use std::path::Path;

pub fn cover(path: &Path, w: u32, h: u32) -> Option<Vec<u8>> {
    let (src, sw, sh) = decode(path)?;
    if sw == 0 || sh == 0 {
        return None;
    }

    // what the scan shows, without the clear margin a disc's scan leaves round the disc, so the
    // disc is the face's, in the middle of it
    let (bx, by, bw, bh) = shown_bounds(&src, sw, sh);
    let scale = (w as f32 / bw as f32).max(h as f32 / bh as f32);
    let ox = bx as f32 + (bw as f32 - w as f32 / scale) / 2.0;
    let oy = by as f32 + (bh as f32 - h as f32 / scale) / 2.0;

    let mut out = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        let y0 = oy + y as f32 / scale;
        let y1 = oy + (y + 1) as f32 / scale;
        for x in 0..w {
            let x0 = ox + x as f32 / scale;
            let x1 = ox + (x + 1) as f32 / scale;
            let px = box_average(&src, sw, sh, x0, y0, x1, y1);
            out[((y * w + x) * 4) as usize..][..4].copy_from_slice(&px);
        }
    }
    Some(out)
}

/// The part of the `sw` by `sh` RGBA `src` that is not clear, as `x, y, w, h`: all of it for a
/// scan with nothing clear at its edges.
fn shown_bounds(src: &[u8], sw: u32, sh: u32) -> (u32, u32, u32, u32) {
    const SHOWN: u8 = 16;
    let (mut x0, mut y0, mut x1, mut y1) = (sw, sh, 0, 0);
    for y in 0..sh {
        for x in 0..sw {
            if src[((y * sw + x) * 4 + 3) as usize] > SHOWN {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
            }
        }
    }
    if x1 <= x0 || y1 <= y0 {
        return (0, 0, sw, sh);
    }
    (x0, y0, x1 - x0, y1 - y0)
}

/// The png at `path` as RGBA, only if it is exactly `w` by `h`.
pub fn exact(path: &Path, w: u32, h: u32) -> Option<Vec<u8>> {
    let (rgba, sw, sh) = decode(path)?;
    ((sw, sh) == (w, h)).then_some(rgba)
}

/// How big the png at `path` is, read from its header alone.
pub fn size(path: &Path) -> Option<(u32, u32)> {
    let file = std::fs::File::open(path).ok()?;
    let reader = png::Decoder::new(std::io::BufReader::new(file))
        .read_info()
        .ok()?;
    Some((reader.info().width, reader.info().height))
}

/// `rgba`, `w` by `h`, as a png: without its alpha when every pixel is opaque, as a cart's label
/// is, and each row filtered as suits it. Compressing harder saves under one byte in a hundred
/// on a label and takes a third as long again.
pub fn encode(rgba: &[u8], w: u32, h: u32) -> Option<Vec<u8>> {
    let opaque = rgba.chunks_exact(4).all(|px| px[3] == 255);
    let rgb: Vec<u8>;
    let (colour, data) = if opaque {
        rgb = rgba
            .chunks_exact(4)
            .flat_map(|px| &px[..3])
            .copied()
            .collect();
        (png::ColorType::Rgb, &rgb[..])
    } else {
        (png::ColorType::Rgba, rgba)
    };
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, w, h);
    enc.set_color(colour);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_adaptive_filter(png::AdaptiveFilterType::Adaptive);
    let mut writer = enc.write_header().ok()?;
    writer.write_image_data(data).ok()?;
    writer.finish().ok()?;
    Some(out)
}

fn box_average(src: &[u8], sw: u32, sh: u32, x0: f32, y0: f32, x1: f32, y1: f32) -> [u8; 4] {
    let xa = (x0.floor().max(0.0) as u32).min(sw - 1);
    let ya = (y0.floor().max(0.0) as u32).min(sh - 1);
    let xb = ((x1.ceil() as u32).max(xa + 1)).min(sw);
    let yb = ((y1.ceil() as u32).max(ya + 1)).min(sh);

    let mut acc = [0u32; 4];
    let mut n = 0u32;
    for y in ya..yb {
        for x in xa..xb {
            let i = ((y * sw + x) * 4) as usize;
            for c in 0..4 {
                acc[c] += src[i + c] as u32;
            }
            n += 1;
        }
    }
    let mut out = [0u8; 4];
    for c in 0..4 {
        out[c] = ((acc[c] + n / 2) / n) as u8;
    }
    out
}

/// The png at `path` as RGBA, with its width and height.
pub(crate) fn decode(path: &Path) -> Option<(Vec<u8>, u32, u32)> {
    let file = std::fs::File::open(path).ok()?;
    let mut dec = png::Decoder::new(std::io::BufReader::new(file));
    dec.set_transformations(png::Transformations::normalize_to_color8());
    dec.set_limits(png::Limits { bytes: 64 << 20 });

    let mut reader = dec.read_info().ok()?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    let src = &buf[..info.buffer_size()];
    let n = (info.width * info.height) as usize;

    let mut rgba = vec![255u8; n * 4];
    match info.color_type {
        png::ColorType::Rgba => rgba.copy_from_slice(src),
        png::ColorType::Rgb => {
            for i in 0..n {
                rgba[i * 4..i * 4 + 3].copy_from_slice(&src[i * 3..i * 3 + 3]);
            }
        }
        png::ColorType::Grayscale => {
            for i in 0..n {
                rgba[i * 4..i * 4 + 3].fill(src[i]);
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for i in 0..n {
                rgba[i * 4..i * 4 + 3].fill(src[i * 2]);
                rgba[i * 4 + 3] = src[i * 2 + 1];
            }
        }
        png::ColorType::Indexed => return None,
    }
    Some((rgba, info.width, info.height))
}
