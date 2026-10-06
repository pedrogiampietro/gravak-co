//! The Action panel of the original (GUI.ini dialog 140, `DialogAct`): the
//! main bar's Action button opens a strip of emotes over the bar, each sent
//! as a ChangeAction.

use crate::action::Action;
use crate::assets::{Assets, Result};
use crate::ui::UiRect;
use image::{imageops, RgbaImage};

/// [0-140]: 458 by 56 at (283, 628) of the 1024 by 768 screen, whose main
/// bar ([0-130]) starts at 627.
pub const SIZE: [u32; 2] = [458, 56];
const OFFSET: [f32; 2] = [283.0, 1.0];

/// [140-1536..1541], [140-1088], [140-1726], [140-1090], [140-1091]: the
/// emotes left to right, 34 by 34 from x 20 every 41, with their art
/// (`Control.Ani` `ButtonA<code>` is `Act<n>`).
pub const EMOTES: [Action; 10] = [
    Action::Cool,
    Action::Kneel,
    Action::Sad,
    Action::Happy,
    Action::Angry,
    Action::Lie,
    Action::Dance,
    Action::Wave,
    Action::Bow,
    Action::Sit,
];
const EMOTE_SIZE: i32 = 34;
const EMOTE_STEP: i32 = 41;
/// [140-1180]: the close button.
const CLOSE: UiRect = UiRect::new(433, 7, 12, 12);

pub fn position(viewport: [f32; 2]) -> [f32; 2] {
    let [left, top] = crate::original_ui_layout::main_hud_position(viewport);
    [left + OFFSET[0], top + OFFSET[1]]
}

fn emote_rect(index: usize) -> UiRect {
    UiRect::new(20 + index as i32 * EMOTE_STEP, 10, EMOTE_SIZE, EMOTE_SIZE)
}

/// What a click on the panel asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionPanelClick {
    Emote(Action),
    Close,
}

/// The click under the cursor; `Some(None)` on the panel itself, `None` off
/// it.
pub fn click_at(viewport: [f32; 2], cursor: [f32; 2]) -> Option<Option<ActionPanelClick>> {
    let [x, y] = position(viewport);
    let point = [cursor[0] - x, cursor[1] - y];
    if !UiRect::new(0, 0, SIZE[0] as i32, SIZE[1] as i32).contains_local(point) {
        return None;
    }
    if CLOSE.contains_local(point) {
        return Some(Some(ActionPanelClick::Close));
    }
    Some(
        (0..EMOTES.len())
            .find(|&index| emote_rect(index).contains_local(point))
            .map(|index| ActionPanelClick::Emote(EMOTES[index])),
    )
}

/// The emote under the cursor, for its pressed art.
pub fn hovered(viewport: [f32; 2], cursor: [f32; 2]) -> Option<usize> {
    match click_at(viewport, cursor)?? {
        ActionPanelClick::Emote(emote) => EMOTES.iter().position(|action| *action == emote),
        ActionPanelClick::Close => None,
    }
}

fn art(assets: &mut Assets, path: &str) -> Result<RgbaImage> {
    crate::original_ui::visible_part(&assets.image(path)?)
}

/// The panel with the emote under the cursor pressed.
pub fn image(assets: &mut Assets, hovered: Option<usize>) -> Result<RgbaImage> {
    let source = assets.image("data/main/DialogAct.dds")?;
    let mut image =
        imageops::crop_imm(&source, 0, 0, SIZE[0].min(source.width()), SIZE[1].min(source.height())).to_image();
    for index in 0..EMOTES.len() {
        let pressed = if hovered == Some(index) { "Click" } else { "" };
        let icon = art(assets, &format!("data/main/Act{index}{pressed}.dds"))?;
        let rect = emote_rect(index);
        imageops::overlay(&mut image, &icon, i64::from(rect.x), i64::from(rect.y));
    }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emotes_sit_in_a_row_over_the_bar() {
        let viewport = [1024.0, 768.0];
        assert_eq!(position(viewport), [283.0, 628.0]);
        assert_eq!(click_at(viewport, [283.0 + 25.0, 628.0 + 20.0]), Some(Some(ActionPanelClick::Emote(Action::Cool))));
        assert_eq!(click_at(viewport, [283.0 + 400.0, 628.0 + 20.0]), Some(Some(ActionPanelClick::Emote(Action::Sit))));
        assert_eq!(click_at(viewport, [283.0 + 438.0, 628.0 + 10.0]), Some(Some(ActionPanelClick::Close)));
        assert_eq!(click_at(viewport, [283.0 + 5.0, 628.0 + 50.0]), Some(None));
        assert_eq!(click_at(viewport, [100.0, 100.0]), None);
    }

    /// `BOUNTY_PREVIEW=dir cargo test -p conquer-sandbox action_panel_preview`
    #[test]
    fn action_panel_preview() {
        let Ok(dir) = std::env::var("BOUNTY_PREVIEW") else { return };
        let mut assets = Assets::open(std::path::Path::new("R:/5065")).expect("assets");
        let source = assets.image("data/main/DialogAct.dds").expect("art");
        println!("DialogAct {}x{}", source.width(), source.height());
        image(&mut assets, Some(3)).expect("panel").save(format!("{dir}/action-panel.png")).expect("png");
    }
}
