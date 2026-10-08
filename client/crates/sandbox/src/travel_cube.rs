//! The Travel Cube's window (`special_items.rs` on the server): the cities
//! it carries its owner to, and how long it rests between two travels (ten
//! minutes). Using the cube from the bag or an F key opens it.

use crate::components::{draw_card, fill, outline, WidgetState};
use crate::ui::UiRect;
use crate::{labels, ui_theme};
use fontdue::Font;
use image::RgbaImage;

/// The cube's item type (`content/items/travel_cube.toml`).
pub const TRAVEL_CUBE: u32 = 723_990;

/// The window's size.
pub const SIZE: [u32; 2] = [420, 420];
const ROW_TOP: i32 = 84;
const ROW: i32 = 44;

/// The window as it is drawn.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct TravelCubeView {
    /// The cube used.
    pub uid: u32,
    /// Seconds until it travels again; 0 ready. `None` until the server
    /// has said.
    pub wait: Option<u32>,
    pub hover: Option<CubeAction>,
}

/// A click in the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CubeAction {
    /// Travel to the city of `crate::store_panel::CITIES` at this index.
    Travel(usize),
    Close,
}

fn row_rect(index: usize) -> UiRect {
    UiRect::new(18, ROW_TOP + index as i32 * ROW, SIZE[0] as i32 - 36, ROW - 6)
}

fn travel_rect(index: usize) -> UiRect {
    let row = row_rect(index);
    UiRect::new(row.x + row.width - 96, row.y + 6, 86, row.height - 12)
}

fn close_rect() -> UiRect {
    UiRect::new(SIZE[0] as i32 - 31, 6, 27, 30)
}

/// What a click at `local` (from the window's corner) does.
pub fn action_at(view: &TravelCubeView, local: [f32; 2]) -> Option<CubeAction> {
    if close_rect().contains_local(local) {
        return Some(CubeAction::Close);
    }
    if view.wait != Some(0) {
        return None;
    }
    (0..crate::store_panel::CITIES.len())
        .find(|index| row_rect(*index).contains_local(local))
        .map(CubeAction::Travel)
}

/// Whether `local` is over the window.
pub fn covers(local: [f32; 2]) -> bool {
    local[0] >= 0.0 && local[1] >= 0.0 && local[0] < SIZE[0] as f32 && local[1] < SIZE[1] as f32
}

fn text(image: &mut RgbaImage, font: &Font, words: &str, colour: [u8; 3], x: i32, y: i32, size: f32) {
    labels::text_sized(image, font, &[(words, colour)], [x as f32, y as f32], size);
}

fn centred(image: &mut RgbaImage, font: &Font, words: &str, colour: [u8; 3], rect: UiRect, size: f32) {
    let width = crate::daily_rewards_panel::width_of(font, words, size) as i32;
    text(image, font, words, colour, rect.x + (rect.width - width) / 2, rect.y + (rect.height - size as i32) / 2 - 1, size);
}

/// "7:05" for 425 seconds.
pub fn clock(seconds: u32) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// What each city is, under its name.
fn blurb(city: &str) -> &'static str {
    match city {
        "Twin City" => "The capital, by the central plaza.",
        "Phoenix Castle" => "The castle town.",
        "Ape Mountain" => "Ape City, in the mountains.",
        "Desert City" => "The desert town.",
        "Bird Island" => "The island town.",
        "Market" => "Booths and trade.",
        _ => "",
    }
}

/// The window's contents (the frame goes around them: `shop::framed`).
pub fn draw(image: &mut RgbaImage, font: &Font, view: &TravelCubeView) {
    const GOLD: [u8; 3] = ui_theme::TEXT_GOLD;
    const TEXT: [u8; 3] = ui_theme::TEXT;
    const MUTED: [u8; 3] = ui_theme::TEXT_MUTED;
    text(image, font, "Where shall the cube take you?", GOLD, 18, 48, 12.0);
    let (status, colour) = match view.wait {
        None => ("Asking the cube...".to_owned(), MUTED),
        Some(0) => ("Ready: once every 10 minutes.".to_owned(), [120, 230, 130]),
        Some(wait) => (format!("Resting: {} until the next travel.", clock(wait)), [235, 170, 110]),
    };
    text(image, font, &status, colour, 18, 64, 10.0);
    let ready = view.wait == Some(0);
    for (index, city) in crate::store_panel::CITIES.iter().enumerate() {
        let row = row_rect(index);
        let hovered = ready && view.hover == Some(CubeAction::Travel(index));
        draw_card(image, row, if hovered { WidgetState::Hovered } else { WidgetState::Normal });
        fill(image, UiRect::new(row.x, row.y, 3, row.height), if ready { [150, 90, 240, 255] } else { [70, 70, 90, 255] });
        text(image, font, city, if ready { TEXT } else { MUTED }, row.x + 14, row.y + 6, 11.0);
        text(image, font, blurb(city), MUTED, row.x + 14, row.y + 21, 9.0);
        let button = travel_rect(index);
        let (fill_colour, border) = match (ready, hovered) {
            (true, true) => ([70, 44, 110, 255], [210, 160, 255, 255]),
            (true, false) => ([40, 26, 66, 255], [150, 90, 240, 255]),
            (false, _) => ([26, 26, 32, 255], [70, 70, 90, 255]),
        };
        fill(image, button, fill_colour);
        outline(image, button, border);
        let label = if ready { "Travel" } else { "Resting" };
        centred(image, font, label, if ready { [235, 220, 255] } else { MUTED }, button, 10.0);
    }
    let foot = SIZE[1] as i32 - 58;
    text(image, font, "It stays with you on death; only a trade passes it on.", MUTED, 18, foot, 9.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_resting_cube_takes_no_travel() {
        let centre = |rect: UiRect| [rect.x as f32 + rect.width as f32 / 2.0, rect.y as f32 + rect.height as f32 / 2.0];
        let mut view = TravelCubeView { uid: 7, wait: Some(0), hover: None };
        assert_eq!(action_at(&view, centre(travel_rect(5))), Some(CubeAction::Travel(5)));
        assert_eq!(action_at(&view, centre(row_rect(1))), Some(CubeAction::Travel(1)));
        assert_eq!(action_at(&view, centre(close_rect())), Some(CubeAction::Close));
        view.wait = Some(30);
        assert_eq!(action_at(&view, centre(travel_rect(5))), None);
        view.wait = None;
        assert_eq!(action_at(&view, centre(travel_rect(5))), None, "not before the server says");
        assert_eq!(clock(425), "7:05");
        let last = row_rect(crate::store_panel::CITIES.len() - 1);
        assert!(last.y + last.height < SIZE[1] as i32 - 48, "the rows fit above the foot");
    }

    /// `HELPER_PREVIEW=dir cargo test -p conquer-sandbox travel_cube_preview`
    #[test]
    fn travel_cube_preview() {
        let Ok(dir) = std::env::var("HELPER_PREVIEW") else { return };
        let mut assets = crate::assets::Assets::open(std::path::Path::new("R:/5065")).expect("assets");
        let font = crate::labels::font().expect("font");
        for (name, wait, hover) in [("ready", Some(0), Some(CubeAction::Travel(1))), ("resting", Some(425), None)] {
            let view = TravelCubeView { uid: 1, wait, hover };
            let mut content = RgbaImage::new(SIZE[0], SIZE[1]);
            draw(&mut content, &font, &view);
            let framed = crate::shop::framed(&mut assets, content, "Travel Cube", false).expect("frame");
            framed.save(format!("{dir}/travel-cube-{name}.png")).expect("png");
        }
    }
}
