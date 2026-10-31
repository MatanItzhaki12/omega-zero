//! Deleting a save state from the switcher over a game. X on the save shown asks first: a DELETE
//! STATE panel over its screenshot says how old it is, and the bar's actions turn to B CANCEL
//! and A DELETE. A deletes it, its picture with it, and the switcher shows the one after it, or
//! goes back to the game when it was the last; B, or X again, keeps it.

use omega_input::{Action, Btn};
use omega_ui::kit::palette::Palette;
use omega_ui::kit::panel::panel_face;
use omega_ui::kit::pixel::LINE;
use omega_ui::kit::row::panel_height;
use omega_ui::{Draw, Toast, UndoFace};

use crate::app::App;
use crate::pause_menu::PendingUndo;
use crate::screens::lone::{notes, Lone, NOTE_GAP};
use crate::screens::power::WIDTH;

/// The panel's title.
pub const TITLE: &str = "DELETE STATE";

/// The question the panel asks, over the age of the save it is about.
pub const QUESTION: &str = "Delete this state?";

/// The panel asking whether to delete the save `age` old, in `pal`: the question, and the age
/// under it, in the dim colour as a note.
pub fn delete_face(age: &str, pal: &Palette) -> UndoFace {
    let lines = [
        QUESTION.to_string(),
        format!("Saved {}", age.to_lowercase()),
    ];
    let mut panel = panel_face(
        WIDTH,
        panel_height(NOTE_GAP + lines.len() as u32 * LINE + 5),
        TITLE,
        true,
        pal,
    );
    notes(&mut panel, &lines, NOTE_GAP, pal);
    panel.face
}

impl App {
    /// Whether the switcher is asking to delete the save it shows.
    pub fn delete_asked(&self) -> bool {
        self.delete_asked
    }

    pub fn set_delete_panel(&mut self, panel: Lone) {
        self.delete_panel = Some(panel);
    }

    /// X on the switcher: the question goes up over the save shown, or comes down again.
    pub(crate) fn ask_delete(&mut self) {
        if self.polaroids.as_ref().is_some_and(|p| !p.is_empty()) {
            self.delete_asked = !self.delete_asked;
        }
    }

    /// A press while the question is up: A deletes, B or X keeps the save.
    pub(crate) fn delete_input(&mut self, action: Action) {
        match action {
            Action::GbaDown(Btn::A) => self.delete_selected(),
            Action::GbaDown(Btn::B | Btn::X) => self.delete_asked = false,
            _ => {}
        }
    }

    /// Deletes the save shown, its picture with it; an Undo Save of that very save goes with it.
    /// The switcher shows the save after it, or the game when there is none left.
    fn delete_selected(&mut self) {
        self.delete_asked = false;
        let (Some(ring), Some(stamp)) = (
            self.ring(),
            self.polaroids
                .as_ref()
                .and_then(|p| p.selected())
                .map(|e| e.stamp.clone()),
        ) else {
            return self.refuse();
        };
        if let Err(e) = ring.remove(&stamp) {
            eprintln!("omega-zero: delete: {e}");
            return self.refuse();
        }
        let undoes_it = matches!(
            &self.pending_undo,
            Some((PendingUndo::Save { stamp: saved, .. }, _)) if *saved == stamp
        );
        if undoes_it {
            self.pending_undo = None;
        }
        self.hud.toast(Toast::StateDeleted, self.now());
        let left = self.polaroids.as_mut().map_or(0, |p| {
            p.remove_selected();
            p.entries.len()
        });
        if left == 0 {
            self.close_polaroids();
        }
    }

    /// The question over the switcher, while it is up.
    pub(crate) fn draw_delete(&self, out: &mut Vec<Draw>) {
        if let (true, Some(panel)) = (self.delete_asked, &self.delete_panel) {
            panel.draw(out);
        }
    }
}
