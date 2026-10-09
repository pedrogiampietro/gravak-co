//! Leaving the game, as the original does (Conquer.exe 0x4751c5, 0x477844,
//! 0x41ce12): Escape closes the open windows one at a time, and with none
//! left asks in its confirmation dialog ([0-158] `CDlgConfirm`, its 27th
//! use) "Are you sure you want to exit?" (StrRes 11000), with OK, Cancel
//! and the Exercise button (环保练功, [158-1463]), which sends the hero to
//! offline training before the game closes. Closing the window and the
//! Options window's Exit ask the same.

use crate::assets::{Assets, Result};
use image::{Rgba, RgbaImage};

/// [0-158]: the dialog's size.
pub const SIZE: [i32; 2] = [300, 108];
/// StrRes 11000.
const QUESTION: &str = "Are you sure you want to exit?";

/// [158-1463], [158-1665] and [158-1664].
const EXERCISE: [i32; 4] = [30, 67, 75, 25];
const OK: [i32; 4] = [145, 67, 36, 19];
const CANCEL: [i32; 4] = [215, 67, 36, 19];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ExitButton {
    Exercise,
    Ok,
    Cancel,
}

/// The open dialog: where it stands, and whether its Exercise is waiting
/// for the server.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExitConfirm {
    /// Top-left corner in window pixels.
    pub position: [i32; 2],
    pub exercising: bool,
}

impl ExitConfirm {
    /// Centred in the window, as the original places [0-158] (400, 300 on
    /// its 1024x768 screen: a little below the middle).
    pub fn new(viewport: [f32; 2]) -> Self {
        let x = ((viewport[0] - SIZE[0] as f32) * 0.5).max(0.0) as i32;
        let y = ((viewport[1] - SIZE[1] as f32) * 0.5 + 30.0).max(0.0) as i32;
        Self { position: [x, y], exercising: false }
    }

    fn local(&self, cursor: [f32; 2]) -> [i32; 2] {
        [cursor[0] as i32 - self.position[0], cursor[1] as i32 - self.position[1]]
    }

    pub fn button_at(&self, cursor: [f32; 2]) -> Option<ExitButton> {
        let [x, y] = self.local(cursor);
        [(EXERCISE, ExitButton::Exercise), (OK, ExitButton::Ok), (CANCEL, ExitButton::Cancel)]
            .into_iter()
            .find(|([left, top, width, height], _)| (*left..left + width).contains(&x) && (*top..top + height).contains(&y))
            .map(|(_, button)| button)
    }

    /// The dialog's art, its question, and its buttons (the hovered one
    /// in its lit frame).
    pub fn image(&self, assets: &mut Assets, hovered: Option<ExitButton>) -> Result<RgbaImage> {
        let mut image = RgbaImage::from_pixel(SIZE[0] as u32, SIZE[1] as u32, Rgba([0, 0, 0, 0]));
        // The art is half see-through: darken what lies under it.
        crate::original_panels::fill(&mut image, [0, 0, SIZE[0], SIZE[1]], [0, 0, 0, 150]);
        art(&mut image, assets, "data/main/DialogConfirm.dds", [0, 0])?;
        if let Ok(font) = crate::main_hud::font() {
            let width = crate::main_hud::text_width(font, QUESTION, 12.0).min(275);
            crate::main_hud::text(&mut image, font, QUESTION, 12.0, [5 + (275 - width) / 2, 11], [255, 255, 255, 255], Some(275));
        }
        // Exercise: normal, pressed and lit frames; OK and Cancel light
        // with their pressed frame.
        let exercise = match hovered {
            Some(ExitButton::Exercise) => "data/interface/Style01/Exercise/ExerciseH.dds",
            _ => "data/interface/Style01/Exercise/Exercise.dds",
        };
        art(&mut image, assets, exercise, [EXERCISE[0], EXERCISE[1]])?;
        let ok = if hovered == Some(ExitButton::Ok) { "data/main/ConfirmOkBtnClick.dds" } else { "data/main/ConfirmOkBtn.dds" };
        art(&mut image, assets, ok, [OK[0], OK[1]])?;
        let cancel =
            if hovered == Some(ExitButton::Cancel) { "data/main/ConfirmCancelBtnClick.dds" } else { "data/main/ConfirmCancelBtn.dds" };
        art(&mut image, assets, cancel, [CANCEL[0], CANCEL[1]])?;
        Ok(image)
    }
}

fn art(image: &mut RgbaImage, assets: &mut Assets, path: &str, [x, y]: [i32; 2]) -> Result<()> {
    crate::original_ui::overlay_asset(image, assets, path, x.into(), y.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn its_buttons_stand_where_the_original_puts_them() {
        let dialog = ExitConfirm { position: [100, 50], exercising: false };
        assert_eq!(dialog.button_at([100.0 + 40.0, 50.0 + 70.0]), Some(ExitButton::Exercise));
        assert_eq!(dialog.button_at([100.0 + 150.0, 50.0 + 70.0]), Some(ExitButton::Ok));
        assert_eq!(dialog.button_at([100.0 + 220.0, 50.0 + 70.0]), Some(ExitButton::Cancel));
        assert_eq!(dialog.button_at([100.0 + 10.0, 50.0 + 10.0]), None);
    }

    /// `GRAVAK_EXIT_PREVIEW=dir cargo test -p conquer-sandbox exit_preview`:
    /// the dialog as drawn, plain and with each button lit.
    #[test]
    fn exit_preview() {
        let Ok(dir) = std::env::var("GRAVAK_EXIT_PREVIEW") else { return };
        let mut assets = Assets::open(std::path::Path::new("R:/5065")).expect("assets");
        let dialog = ExitConfirm { position: [0, 0], exercising: false };
        for (name, hovered) in [("plain", None), ("exercise", Some(ExitButton::Exercise)), ("ok", Some(ExitButton::Ok))] {
            dialog.image(&mut assets, hovered).expect("image").save(format!("{dir}/exit-{name}.png")).expect("png");
        }
    }
}
