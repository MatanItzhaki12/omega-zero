//! The MENU panel's face, made while it is open, when what it shows or the palette does.

use omega_gfx::Compositor;

use crate::app::App;
use crate::frontend::lone::Baked;
use crate::pause_menu::{pause_panel_face, PauseView};

pub(crate) type PauseTex = Baked<(PauseView, u8)>;

pub(crate) fn sync_pause(app: &mut App, compositor: &mut Compositor, tex: &mut PauseTex) {
    let Some(view) = app.pause_view() else {
        return;
    };
    let (colour, pal) = (app.palette_colour(), app.palette());
    if let Some(panel) = tex.bake(compositor, (view, colour), |(view, _)| {
        pause_panel_face(view, pal)
    }) {
        app.set_pause_panel(panel);
    }
}
