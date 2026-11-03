//! The PSP's UMD: a disc 60 mm across in a shell 65 by 64 mm and 4.2 thick, its top corners
//! rounded wide about the disc and its foot square. The shell is smoky clear plastic, so the
//! disc shows through it all round the label: its metal hub, the clear ring about it, and its
//! silver face out to its edge, which catches the light as the UMD turns, the way a label's
//! holo sheen does.
//!
//! The UMD is drawn 4 units to the millimetre and its face about 4.25 pixels to it.

use std::f32::consts::PI;
use std::sync::OnceLock;

use crate::cart::{Inside, Spec, PAD};
use crate::silhouette::{depth_map, rasterise_detail, rasterise_svg, Detail};

/// How thick the UMD is, as a share of its width: 4.2 mm on 65.
pub const THICK: f32 = 0.065;

/// How big the UMD stands on the shelf, as a share of its own size.
pub const SHELF_SCALE: f32 = 1.0;

/// The drawing's size in units: 65 by 64 mm.
const UNITS: (f32, f32) = (260.0, 256.0);

/// How round the top corners are, in units: 22 mm, so the top follows the disc inside.
const TOP_R: f32 = 88.0;
/// How round the foot's corners are, in units.
const FOOT_R: f32 = 10.0;

/// The cuts each corner of the outline is made of.
const TOP_CUTS: usize = 6;
const FOOT_CUTS: usize = 2;

/// The face in pixels: each an even number, so its halves stand on whole art pixels.
pub const UMD_W: u32 = 276;
pub const UMD_H: u32 = 272;

/// The label on the front, in the face's pixels: across the shell from just under its top to
/// a little below the disc's middle, its corners round.
const LABEL: (u32, u32, u32, u32) = (22, 20, 232, 168);
const LABEL_ROUND: u32 = 18;

/// How far in from the outline the clear shell lightens at its rim, in pixels.
const RIM: u32 = 6;

/// Pixels to the millimetre on the face.
const PX_MM: f32 = UMD_W as f32 / 65.0;

/// The disc's middle in the face's pixels: in the middle across, and as high in the shell as
/// it is far in from the sides.
const DISC_MIDDLE: (f32, f32) = (UMD_W as f32 / 2.0, 134.0);

// The rings of the disc, in millimetres out from its middle: the spindle's hole, the metal hub
// the drive clamps, the clear ring about it, and the silver face out to the edge.
const SPINDLE: f32 = 2.6;
const HUB: f32 = 7.5;
const CLEAR: f32 = 11.0;
const FACE: f32 = 29.3;
const EDGE: f32 = 30.0;

const SPINDLE_DARK: [u8; 3] = [0x26, 0x28, 0x2e];
const HUB_METAL: [u8; 3] = [0xa4, 0xa8, 0xb2];
const HUB_RIM: [u8; 3] = [0x6c, 0x70, 0x7a];
const FACE_DARK: [u8; 3] = [0x9a, 0xa0, 0xae];
const FACE_LIT: [u8; 3] = [0xe0, 0xe4, 0xec];
const DISC_EDGE: [u8; 3] = [0x80, 0x84, 0x8e];

/// What a part of the disc shows behind the face's pixel `x, y`.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum DiscPart {
    Spindle,
    Hub,
    /// The clear ring about the hub, which shows the hollow shell as plastic does.
    Clear,
    Face,
    Edge,
}

/// How far the middle of the art pixel holding `x, y` is from the disc's middle, in mm, and at
/// what angle.
fn polar(x: u32, y: u32) -> (f32, f32) {
    let (cx, cy) = ((x & !1) as f32 + 1.0, (y & !1) as f32 + 1.0);
    let (dx, dy) = (cx - DISC_MIDDLE.0, cy - DISC_MIDDLE.1);
    ((dx * dx + dy * dy).sqrt() / PX_MM, dy.atan2(dx))
}

/// The part of the disc behind `x, y`, if any.
pub(crate) fn disc_part(x: u32, y: u32) -> Option<DiscPart> {
    let (d, _) = polar(x, y);
    Some(match d {
        d if d < SPINDLE => DiscPart::Spindle,
        d if d < HUB => DiscPart::Hub,
        d if d < CLEAR => DiscPart::Clear,
        d if d < FACE => DiscPart::Face,
        d if d < EDGE => DiscPart::Edge,
        _ => return None,
    })
}

/// How far out on the disc's silver face `x, y` is, 0 at its inner edge and 1 at its outer,
/// for its rainbow.
pub(crate) fn out_on_face(x: u32, y: u32) -> f32 {
    ((polar(x, y).0 - CLEAR) / (FACE - CLEAR)).clamp(0.0, 1.0)
}

/// What stands behind `x, y` in the shell: the disc's colour there, or nothing.
pub(crate) fn behind(x: u32, y: u32) -> Option<[u8; 3]> {
    let (d, angle) = polar(x, y);
    match disc_part(x, y)? {
        DiscPart::Spindle => Some(SPINDLE_DARK),
        DiscPart::Hub if HUB - d < 0.5 => Some(HUB_RIM),
        DiscPart::Hub => Some(HUB_METAL),
        DiscPart::Clear => None,
        DiscPart::Edge => Some(DISC_EDGE),
        // a mirror catches the light in two wedges across from each other, top left and bottom
        // right, dithered a pixel in two at their edges, as the PlayStation's disc does
        DiscPart::Face => {
            let off = (angle - PI / 4.0).rem_euclid(PI);
            let off = off.min(PI - off).to_degrees();
            let dither = ((x / 2) + (y / 2)).is_multiple_of(2);
            Some(match off < 10.0 || (off < 20.0 && dither) {
                true => FACE_LIT,
                false => FACE_DARK,
            })
        }
    }
}

/// One rounded corner of the outline, `cuts` cuts from `from` to `to` radians about `centre`,
/// `r` units out: the corners between its ends stand a little outside the arc and the cuts
/// between them a little inside, so the walls follow the edge the face's alpha cuts.
fn corner(out: &mut Vec<(f32, f32)>, centre: (f32, f32), r: f32, from: f32, to: f32, cuts: usize) {
    let out_by = (1.0 + 1.0 / ((to - from) / cuts as f32 / 2.0).cos()) / 2.0;
    for i in 0..=cuts {
        let a = from + (to - from) * i as f32 / cuts as f32;
        let r = if i == 0 || i == cuts { r } else { r * out_by };
        out.push((centre.0 + r * a.cos(), centre.1 + r * a.sin()));
    }
}

/// The UMD's outline in drawing units, all the way round clockwise from the top, and the
/// drawing's size.
pub(crate) fn outline() -> (Vec<(f32, f32)>, (f32, f32)) {
    let (w, h) = UNITS;
    let mut all = Vec::new();
    corner(
        &mut all,
        (w - TOP_R, TOP_R),
        TOP_R,
        -PI / 2.0,
        0.0,
        TOP_CUTS,
    );
    corner(
        &mut all,
        (w - FOOT_R, h - FOOT_R),
        FOOT_R,
        0.0,
        PI / 2.0,
        FOOT_CUTS,
    );
    corner(
        &mut all,
        (FOOT_R, h - FOOT_R),
        FOOT_R,
        PI / 2.0,
        PI,
        FOOT_CUTS,
    );
    corner(&mut all, (TOP_R, TOP_R), TOP_R, PI, 1.5 * PI, TOP_CUTS);
    (all, UNITS)
}

/// The outline as an SVG path, its corners true arcs.
fn outline_path() -> String {
    let (w, h) = UNITS;
    format!(
        "M {t} 0 L {a} 0 A {t} {t} 0 0 1 {w} {t} L {w} {b} A {f} {f} 0 0 1 {c} {h} \
         L {f} {h} A {f} {f} 0 0 1 0 {b} L 0 {t} A {t} {t} 0 0 1 {t} 0 Z",
        t = TOP_R,
        f = FOOT_R,
        a = w - TOP_R,
        b = h - FOOT_R,
        c = w - FOOT_R,
    )
}

fn svg(body: &str) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\">{body}</svg>",
        UNITS.0, UNITS.1
    )
}

/// The relief moulded in the shell: its edges, the top and left catching the light and the
/// right and foot turned from it, and the arrow under the label that says which way it goes in.
fn detail_svg() -> String {
    let (w, h) = UNITS;
    let path = outline_path();
    svg(&format!(
        "<clipPath id=\"c\"><path d=\"{path}\"/></clipPath><g clip-path=\"url(#c)\">\
         <path d=\"{path}\" fill=\"none\" stroke=\"#fff\" stroke-opacity=\"0.4\" stroke-width=\"8\"/>\
         <rect x=\"{r}\" y=\"{t}\" width=\"5\" height=\"{side}\" fill=\"#000\" fill-opacity=\"0.4\"/>\
         <rect x=\"0\" y=\"{fy}\" width=\"{w}\" height=\"5\" fill=\"#000\" fill-opacity=\"0.35\"/>\
         </g>\
         <path d=\"M 130 196 L 140 212 L 120 212 Z\" fill=\"#000\" fill-opacity=\"0.45\"/>\
         <path d=\"M 120.5 212 L 139.5 212 L 140 214 L 120 214 Z\" fill=\"#fff\" fill-opacity=\"0.5\"/>",
        r = w - 5.0,
        t = TOP_R,
        side = h - TOP_R,
        fy = h - 5.0,
    ))
}

fn mask() -> &'static [u8] {
    static MASK: OnceLock<Vec<u8>> = OnceLock::new();
    MASK.get_or_init(|| {
        let shape = svg(&format!("<path d=\"{}\" fill=\"#fff\"/>", outline_path()));
        rasterise_svg(&shape, UMD_W, UMD_H).unwrap_or_else(|| vec![255; (UMD_W * UMD_H) as usize])
    })
}

fn depth() -> &'static [u8] {
    static DEPTH: OnceLock<Vec<u8>> = OnceLock::new();
    DEPTH.get_or_init(|| depth_map(mask(), UMD_W as usize, UMD_H as usize))
}

fn detail() -> &'static Detail {
    static DETAIL: OnceLock<Detail> = OnceLock::new();
    DETAIL.get_or_init(|| {
        rasterise_detail(&detail_svg(), UMD_W, UMD_H).unwrap_or_else(|| Detail::blank(UMD_W, UMD_H))
    })
}

/// How the UMD's face is laid out: what a clear shell shows inside it is the disc.
pub(crate) fn spec() -> Spec {
    Spec {
        w: UMD_W,
        h: UMD_H,
        label: LABEL,
        label_round: LABEL_ROUND,
        mask: mask(),
        depth: depth(),
        detail: detail(),
        max_h: (LABEL.3 - 2 * PAD) as f32,
        rim: RIM,
        inside: Inside::Disc,
    }
}
