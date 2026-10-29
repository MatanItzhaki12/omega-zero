//! The GAMEPLAY and SHELF panels' face, made while one is open, when what it shows or the
//! palette does; and the picked cart's name over the shelf, while Game Titles shows it.

use omega_gfx::{out_w, Compositor};
use omega_ui::kit::palette::Palette;
use omega_ui::kit::text::text_face;
use omega_ui::{UndoFace, SPRITE_PX};

use crate::app::App;
use crate::frontend::faces::upload_nearest;
use crate::frontend::lone::Baked;
use crate::options::{options_panel_face, OptionsView};
use crate::screens::lone::{fitted, over};

pub(crate) type OptionsTex = Baked<(OptionsView, u8)>;

/// The title's texture, and the name and palette it was made for.
#[derive(Default)]
pub(crate) struct TitleTex {
    tex: Option<omega_gfx::TexId>,
    built: Option<(String, u8)>,
}

pub(crate) fn sync_options(app: &mut App, compositor: &mut Compositor, tex: &mut OptionsTex) {
    let Some(view) = app.options_view() else {
        return;
    };
    let (colour, pal) = (app.palette_colour(), app.palette());
    if let Some(panel) = tex.bake(compositor, (view, colour), |(view, _)| {
        options_panel_face(view, pal)
    }) {
        app.set_options_panel(panel);
    }
}

pub(crate) fn sync_title(app: &mut App, compositor: &mut Compositor, title: &mut TitleTex) {
    let Some(name) = app.shelf_title() else {
        return;
    };
    let colour = app.palette_colour();
    let want = (name, colour);
    if title.built.as_ref() == Some(&want) {
        return;
    }
    let room = out_w() / SPRITE_PX - 32;
    let face = title_face(&fitted(&want.0, room), app.palette());
    let tex = upload_nearest(compositor, &mut title.tex, &face);
    app.set_title_face(tex, face.w, face.h);
    title.built = Some(want);
}

/// `text` in cream over its own shadow a pixel down and right, so it reads over any backdrop.
fn title_face(text: &str, pal: &Palette) -> UndoFace {
    let fg = text_face(text, pal.cream);
    let shadow = text_face(text, [0, 0, 0]);
    let mut face = UndoFace {
        rgba: vec![0; ((fg.w + 1) * (fg.h + 1) * 4) as usize],
        w: fg.w + 1,
        h: fg.h + 1,
    };
    over(&mut face, &shadow, 1, 1);
    over(&mut face, &fg, 0, 0);
    face
}
