//! The save-state switcher's photos, made when it opens, and its SAVE STATES panel, baked again
//! when what it says changes: the selection, an age as time goes by, or the palette. The panel
//! is as wide as the widest age among the saves, so it holds still as the selection moves. The
//! DELETE STATE panel is baked while the switcher asks whether to delete the save shown.

use omega_gfx::{Compositor, TexId};
use omega_store::format_stamp;
use omega_ui::kit::saves::{saves_face, saves_width};
use omega_ui::photo_face;

use crate::app::{App, Phase};
use crate::frontend::faces::upload_nearest;
use crate::frontend::lone::Baked;
use crate::screens::delete_state::{delete_face, delete_hint_face};

#[derive(Default)]
pub(crate) struct Switcher {
    open: bool,
    /// The panel's texture, reused each time the switcher opens.
    panel: Option<TexId>,
    /// What the panel was last baked from, since the switcher opened.
    baked: Option<Panel>,
    /// The DELETE STATE panel, by the age it says and the palette colour.
    delete: Baked<(String, u8)>,
    /// The X DELETE line, by the palette colour.
    hint: Baked<u8>,
}

/// What the panel shows: the age of the save shown, which of how many it is, the panel's width
/// and the palette colour it is drawn in.
#[derive(Clone, PartialEq, Eq, Debug)]
struct Panel {
    age: String,
    count: usize,
    at: usize,
    w: u32,
    colour: u8,
}

pub(crate) fn sync_switcher(
    app: &mut App,
    compositor: &mut Compositor,
    pool: &mut Vec<TexId>,
    state: &mut Switcher,
) {
    if !matches!(app.phase(), Phase::Polaroids { .. }) {
        state.open = false;
        return;
    }
    if !state.open {
        state.open = true;
        state.baked = None;
        let faces: Vec<_> = app.polaroid_entries().iter().map(photo_face).collect();
        let ids = faces
            .iter()
            .enumerate()
            .map(|(i, f)| match pool.get(i) {
                Some(id) => {
                    compositor.update_texture(*id, f.w, f.h, &f.rgba);
                    *id
                }
                None => {
                    let id = compositor.create_texture_nearest(f.w, f.h, &f.rgba);
                    pool.push(id);
                    id
                }
            })
            .collect();
        app.set_polaroid_faces(ids);
    }
    if app.polaroids().is_none() {
        return;
    }
    sync_panel(app, compositor, state);
    let pal = app.palette();
    if let Some(hint) = state
        .hint
        .bake(compositor, app.palette_colour(), |_| delete_hint_face(pal))
    {
        app.set_delete_hint(hint);
    }
    if app.delete_asked() {
        let age = app
            .polaroids()
            .map(|p| p.age(&format_stamp(app.wall_secs())))
            .unwrap_or_default();
        let pal = app.palette();
        let baked = state
            .delete
            .bake(compositor, (age, app.palette_colour()), |(age, _)| {
                delete_face(age, pal)
            });
        if let Some(panel) = baked {
            app.set_delete_panel(panel);
        }
    }
}

/// The SAVE STATES panel, baked again when what it says or the palette changes.
fn sync_panel(app: &mut App, compositor: &mut Compositor, state: &mut Switcher) {
    let Some(p) = app.polaroids() else {
        return;
    };
    let ages = p.ages(&format_stamp(app.wall_secs()));
    let (count, at) = p.dots();
    let panel = Panel {
        w: ages
            .iter()
            .map(|a| saves_width(a, count))
            .max()
            .unwrap_or(0),
        age: ages.get(at).cloned().unwrap_or_default(),
        count,
        at,
        colour: app.palette_colour(),
    };
    if state.baked.as_ref() == Some(&panel) {
        return;
    }
    let face = saves_face(panel.w, &panel.age, count, at, app.palette());
    let tex = upload_nearest(compositor, &mut state.panel, &face);
    app.set_polaroid_panel(tex, face.w, face.h);
    state.baked = Some(panel);
}
