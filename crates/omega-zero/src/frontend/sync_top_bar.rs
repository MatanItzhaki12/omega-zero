//! The top bar's faces: what the handheld is connected to, on and off, the clock, the battery's
//! figure and its bolt, all baked in the top bar's ink (`ChromeColours::ink`). The ink is the
//! palette's, so a new colour in Settings > Theme bakes them all again, as it does the
//! kit's faces; a card's theme.txt keeps them white. The clock and the figure are also printed
//! again when what they say changes. Every one is the kit's, a texel to an art pixel, uploaded
//! NEAREST.

use omega_gfx::{Compositor, TexId};
use omega_ui::kit::text::text_face;
use omega_ui::{bolt_face, clock_text, link_face, Link, LinkFaces, Printed};

use crate::app::{App, Readouts};
use crate::frontend::faces::upload_nearest;
use crate::frontend::lone::Baked;

/// The top bar's textures, each reused as what it shows changes, and what they were baked from.
#[derive(Default)]
pub(crate) struct TopBarTexes {
    /// The ink, on and off, the icons and the bolt were baked in.
    ink: Option<([u8; 3], [u8; 3])>,
    /// Wi-Fi's, Bluetooth's and SSH's faces, each off then on.
    links: [[Option<TexId>; 2]; 3],
    bolt: Option<TexId>,
    clock: Baked<(String, [u8; 3])>,
    percent: Baked<(String, [u8; 3])>,
    /// What the app was last handed.
    readouts: Readouts,
}

pub(crate) fn sync_top_bar(app: &mut App, compositor: &mut Compositor, texes: &mut TopBarTexes) {
    let colours = app.chrome_colours();
    let ink = (colours.ink, colours.ink_off);
    let mut readouts = texes.readouts;
    if texes.ink != Some(ink) {
        let mut pair = |held: &mut [Option<TexId>; 2], link: Link| {
            [false, true].map(|on| {
                let face = link_face(link, on, &colours);
                let tex = upload_nearest(compositor, &mut held[usize::from(on)], &face);
                (tex, face.w, face.h)
            })
        };
        let [wifi, bluetooth, ssh] = &mut texes.links;
        app.set_link_faces(LinkFaces {
            wifi: pair(wifi, Link::Wifi),
            bluetooth: pair(bluetooth, Link::Bluetooth),
            ssh: pair(ssh, Link::Ssh),
        });
        let bolt = bolt_face(colours.ink);
        readouts.bolt = Some(upload_nearest(compositor, &mut texes.bolt, &bolt));
        texes.ink = Some(ink);
    }
    let word = |(text, ink): &(String, [u8; 3])| text_face(text, *ink);
    let time = (clock_text(app.wall_secs(), app.clock_24h()), colours.ink);
    if let Some(clock) = texes.clock.bake(compositor, time, word) {
        readouts.clock = Printed::new(clock.tex, clock.w);
    }
    if let Some(battery) = app.battery() {
        let figure = (format!("{}%", battery.percent), colours.ink);
        if let Some(percent) = texes.percent.bake(compositor, figure, word) {
            readouts.percent = Printed::new(percent.tex, percent.w);
        }
    }
    if readouts != texes.readouts {
        app.set_top_readouts(readouts);
        texes.readouts = readouts;
    }
}
