//! The Highscores window: the categories and professions along the top,
//! the ranking ten to a page with its pages under it, and on the right the
//! hero's own rank and the podium of the first three. Drawn inside a quick
//! panel's frame, in the Reward Wall's style.

use crate::assets::Assets;
use crate::daily_rewards_panel::{
    card, centred, glow_in, paint_in, rounded, rule, say, slot, width_of, GOLD, GOLD_DARK, GOLD_LIGHT,
};
use crate::paint::{self, Colour};
use crate::ui::UiRect;
use crate::{labels, ui_theme};
use fontdue::Font;
use gravak_protocol::{HighscoreRow, HighscoreState};
use image::RgbaImage;

/// The categories, in the server's order (`highscores.rs`), and what their
/// value column says.
pub const CATEGORIES: [(&str, &str); 4] =
    [("Level", "Level"), ("Bounties", "Tasks"), ("Nobility", "Donation"), ("Daily Streak", "Days")];
pub const PROFESSIONS: [&str; 5] = ["All", "Trojan", "Warrior", "Archer", "Taoist"];

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct HighscoresView {
    /// The page's rows.
    pub rows: Vec<HighscoreRow>,
    /// The first three.
    pub podium: Vec<HighscoreRow>,
    pub state: Option<HighscoreState>,
    /// The hero, to mark their row.
    pub own_name: String,
    pub own_mesh: u32,
}

impl HighscoresView {
    /// Adds a row as it arrives; index 0 starts them over.
    pub fn receive(&mut self, row: HighscoreRow) {
        if row.index == 0 {
            self.rows.clear();
            self.podium.clear();
        }
        if row.kind == 1 {
            self.podium.push(row);
        } else {
            self.rows.push(row);
        }
    }

    fn category(&self) -> u32 {
        self.state.as_ref().map_or(0, |state| state.category)
    }

    fn profession(&self) -> u32 {
        self.state.as_ref().map_or(0, |state| state.profession)
    }
}

/// What a click on the Highscores asks for: a page of a category and
/// profession, or the hero's own page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighscoresAction {
    Show { category: u32, profession: u32, page: u32 },
    OwnPage { category: u32, profession: u32 },
}

// The layout, in the panel's 520 x 420 contents.
const TABLE: UiRect = UiRect::new(14, 104, 336, 266);
const HEADER_HEIGHT: i32 = 22;
const ROW_HEIGHT: i32 = 24;
const SIDE_LEFT: i32 = 360;
const SIDE_WIDTH: i32 = 146;
const OWN: UiRect = UiRect::new(SIDE_LEFT, 40, SIDE_WIDTH, 128);
const PODIUM: UiRect = UiRect::new(SIDE_LEFT, 176, SIDE_WIDTH, 194);
const PAGER_Y: i32 = 376;

fn category_tab(index: usize) -> UiRect {
    UiRect::new(14 + index as i32 * 85, 40, 81, 26)
}

fn profession_chip(index: usize) -> UiRect {
    UiRect::new(14 + index as i32 * 68, 72, 64, 24)
}

fn row_rect(row: usize) -> UiRect {
    UiRect::new(TABLE.x + 4, TABLE.y + HEADER_HEIGHT + 4 + row as i32 * ROW_HEIGHT, TABLE.width - 8, ROW_HEIGHT - 2)
}

/// The pager's buttons: first, previous, next, last.
fn pager_button(index: usize) -> UiRect {
    let x = match index {
        0 => 92,
        1 => 124,
        2 => 240,
        _ => 272,
    };
    UiRect::new(x, PAGER_Y, 28, 22)
}

/// A tab or chip: a small rounded plate, gold when chosen.
fn chip(image: &mut RgbaImage, ui: &Font, rect: UiRect, label: &str, chosen: bool, size: f32) {
    let shape = rounded(rect, 5.0);
    if chosen {
        glow_in(image, rect, &shape, 5.0, paint::hex(0xffd36b), 0.3);
    }
    let (top, bottom) = if chosen {
        (paint::hex(0x6a4a16), paint::hex(0x2c1d06))
    } else {
        (paint::hex(0x163346), paint::hex(0x08161f))
    };
    let (y0, height) = (rect.y as f32, rect.height as f32);
    paint_in(image, rect, 1, &shape, |_, y| paint::mix(top, bottom, (y - y0) / height), 0.95);
    let border = if chosen { GOLD_LIGHT } else { paint::hex(0x3d6070) };
    paint_in(image, rect, 1, |x, y| shape(x, y).abs() - 0.6, |_, _| border, 1.0);
    let colour = if chosen { [255, 236, 170] } else { [200, 196, 176] };
    centred(image, ui, label, colour, rect.x + rect.width / 2, rect.y + (rect.height - size as i32) / 2 - 1, size);
}

/// A medal for the first three: gold, silver, bronze.
fn medal(image: &mut RgbaImage, cinzel: &Font, centre: [f32; 2], radius: f32, rank: u32) {
    let (light, mid, dark): (Colour, Colour, Colour) = match rank {
        1 => (GOLD_LIGHT, GOLD, GOLD_DARK),
        2 => (paint::hex(0xf2f4f6), paint::hex(0xa9b3bb), paint::hex(0x56616a)),
        _ => (paint::hex(0xf0c49a), paint::hex(0xb8743c), paint::hex(0x5c3416)),
    };
    let bounds = UiRect::new((centre[0] - radius) as i32 - 2, (centre[1] - radius) as i32 - 2, (radius * 2.0) as i32 + 5, (radius * 2.0) as i32 + 5);
    paint_in(
        image,
        bounds,
        1,
        |x, y| paint::circle(x, y, centre, radius),
        |_, y| paint::gradient(&[(0.0, light), (0.5, mid), (1.0, dark)], (y - centre[1] + radius) / (2.0 * radius)),
        1.0,
    );
    paint_in(image, bounds, 1, |x, y| paint::circle(x, y, centre, radius - 2.5).abs() - 0.5, |_, _| dark, 0.9);
    let size = (radius * 1.05).max(9.0);
    centred(image, cinzel, &rank.to_string(), [30, 20, 6], centre[0] as i32, (centre[1] - size * 0.62) as i32, size);
}

/// A face portrait in a small slot; the slot alone when the face is missing.
fn face(image: &mut RgbaImage, assets: &mut Assets, mesh: u32, rect: UiRect) {
    slot(image, rect, false);
    if let Some(portrait) = crate::renderer::player_face(assets, mesh, (rect.width - 4) as u32) {
        image::imageops::overlay(image, &portrait, i64::from(rect.x + 2), i64::from(rect.y + 2));
    }
}

/// The short name of a class.
fn profession_name(class: u32) -> &'static str {
    match class {
        10..=15 => "Trojan",
        20..=25 => "Warrior",
        40..=45 => "Archer",
        132..=135 => "Water",
        142..=145 => "Fire",
        100..=145 => "Taoist",
        _ => "-",
    }
}

/// `1,234,567`.
fn thousands(value: u64) -> String {
    crate::store_panel::thousands(value)
}

/// What the value column shows for a category.
fn value_text(category: u32, row: &HighscoreRow) -> String {
    match category {
        0 if row.rebirths > 0 => format!("{} ({}R)", row.level, row.rebirths),
        0 => row.level.to_string(),
        2 => thousands(row.value),
        _ => thousands(row.value),
    }
}

/// "7 minutes ago".
fn ago(then: u64, now: u64) -> String {
    let minutes = now.saturating_sub(then) / 60;
    match minutes {
        0 => "just now".to_owned(),
        1 => "1 minute ago".to_owned(),
        minutes => format!("{minutes} minutes ago"),
    }
}

/// Draws the Highscores onto the quick panel's contents.
pub fn draw(image: &mut RgbaImage, assets: &mut Assets, view: Option<&HighscoresView>) {
    let (Ok(ui), Ok(cinzel)) = (labels::font(), labels::decorative_font()) else {
        return;
    };
    let Some((view, state)) = view.and_then(|view| Some((view, view.state.as_ref()?))) else {
        say(image, &ui, "Loading the highscores...", ui_theme::TEXT_MUTED, 24, 56, 14.0);
        return;
    };
    let category = view.category();

    for (index, (name, _)) in CATEGORIES.iter().enumerate() {
        chip(image, &ui, category_tab(index), name, index as u32 == category, 12.0);
    }
    for (index, name) in PROFESSIONS.iter().enumerate() {
        chip(image, &ui, profession_chip(index), name, index as u32 == view.profession(), 11.0);
    }

    // The table: its header, then the page's rows.
    card(image, TABLE, false, false, false);
    let header_y = TABLE.y + 5;
    let value_label = CATEGORIES.get(category as usize).map_or("Value", |(_, label)| label);
    say(image, &ui, "Rank", ui_theme::TEXT_MUTED, TABLE.x + 10, header_y, 11.0);
    say(image, &ui, "Name", ui_theme::TEXT_MUTED, TABLE.x + 82, header_y, 11.0);
    say(image, &ui, "Class", ui_theme::TEXT_MUTED, TABLE.x + 210, header_y, 11.0);
    let label_width = width_of(&ui, value_label, 11.0).round() as i32;
    say(image, &ui, value_label, ui_theme::TEXT_MUTED, TABLE.x + TABLE.width - 12 - label_width, header_y, 11.0);
    rule(image, TABLE.x + 8, TABLE.x + TABLE.width - 8, TABLE.y + HEADER_HEIGHT);
    if view.rows.is_empty() {
        centred(image, &ui, "Nobody is ranked here yet.", ui_theme::TEXT_MUTED, TABLE.x + TABLE.width / 2, TABLE.y + 120, 13.0);
    }
    for (index, row) in view.rows.iter().enumerate() {
        let rect = row_rect(index);
        let own = row.name == view.own_name;
        let shape = rounded(rect, 4.0);
        let fill = if own {
            paint::hex(0x3a2c0e)
        } else if index % 2 == 0 {
            paint::hex(0x0d2230)
        } else {
            paint::hex(0x081820)
        };
        paint_in(image, rect, 1, &shape, |_, _| fill, 0.85);
        if own {
            paint_in(image, rect, 1, |x, y| shape(x, y).abs() - 0.6, |_, _| GOLD, 1.0);
        }
        let middle = rect.y + rect.height / 2;
        if row.rank <= 3 {
            medal(image, &cinzel, [(rect.x + 18) as f32, middle as f32], 9.0, row.rank);
        } else {
            centred(image, &cinzel, &row.rank.to_string(), [220, 210, 180], rect.x + 18, middle - 7, 12.0);
        }
        face(image, assets, row.mesh, UiRect::new(rect.x + 40, rect.y + 1, 20, 20));
        let name_colour = if own { [255, 226, 143] } else { [236, 228, 204] };
        say(image, &ui, &row.name, name_colour, rect.x + 68, middle - 7, 12.0);
        say(image, &ui, profession_name(row.class), ui_theme::TEXT_MUTED, rect.x + 202, middle - 6, 11.0);
        let value = value_text(category, row);
        let value_width = width_of(&ui, &value, 12.0).round() as i32;
        say(image, &ui, &value, [255, 226, 143], rect.x + rect.width - 8 - value_width, middle - 7, 12.0);
    }

    // The pager.
    let page = state.page + 1;
    let pages = state.pages.max(1);
    for (index, label) in ["<<", "<", ">", ">>"].into_iter().enumerate() {
        let usable = if index < 2 { page > 1 } else { page < pages };
        let rect = pager_button(index);
        let shape = rounded(rect, 4.0);
        let fill = if usable { paint::hex(0x163346) } else { paint::hex(0x0a141a) };
        paint_in(image, rect, 1, &shape, |_, _| fill, 0.95);
        paint_in(image, rect, 1, |x, y| shape(x, y).abs() - 0.6, |_, _| if usable { GOLD_DARK } else { paint::hex(0x26343a) }, 1.0);
        let colour = if usable { [255, 226, 143] } else { [90, 96, 96] };
        centred(image, &ui, label, colour, rect.x + rect.width / 2, rect.y + 4, 12.0);
    }
    centred(image, &cinzel, &format!("{page} / {pages}"), [236, 228, 204], 182, PAGER_Y + 3, 13.0);

    // The hero's own rank.
    card(image, OWN, state.own_rank > 0, false, false);
    centred(image, &ui, "Your position", ui_theme::TEXT_MUTED, OWN.x + OWN.width / 2, OWN.y + 8, 11.0);
    face(image, assets, view.own_mesh, UiRect::new(OWN.x + (OWN.width - 46) / 2, OWN.y + 28, 46, 46));
    let rank = if state.own_rank > 0 { format!("#{}", thousands(u64::from(state.own_rank))) } else { "Unranked".to_owned() };
    let rank_size = if state.own_rank > 0 { 20.0 } else { 14.0 };
    centred(image, &cinzel, &rank, [255, 226, 143], OWN.x + OWN.width / 2, OWN.y + 82, rank_size);
    if state.own_rank > 0 {
        centred(image, &ui, "Click to see your page", [150, 160, 156], OWN.x + OWN.width / 2, OWN.y + 108, 10.0);
    }

    // The podium.
    card(image, PODIUM, false, false, true);
    centred(image, &cinzel, "Top 3", [255, 226, 143], PODIUM.x + PODIUM.width / 2, PODIUM.y + 8, 13.0);
    rule(image, PODIUM.x + 10, PODIUM.x + PODIUM.width - 10, PODIUM.y + 28);
    for (index, row) in view.podium.iter().take(3).enumerate() {
        let top = PODIUM.y + 36 + index as i32 * 52;
        medal(image, &cinzel, [(PODIUM.x + 18) as f32, (top + 20) as f32], 11.0, row.rank);
        face(image, assets, row.mesh, UiRect::new(PODIUM.x + 34, top + 2, 36, 36));
        say(image, &ui, &row.name, [236, 228, 204], PODIUM.x + 76, top + 6, 12.0);
        say(image, &ui, &value_text(category, row), [255, 226, 143], PODIUM.x + 76, top + 22, 11.0);
    }

    let update = format!("Last update: {}", ago(state.updated_at, crate::store_panel::now_secs()));
    let update_width = width_of(&ui, &update, 10.0).round() as i32;
    say(image, &ui, &update, ui_theme::TEXT_MUTED, SIDE_LEFT + SIDE_WIDTH - update_width, PAGER_Y + 6, 10.0);
}

/// The action under `cursor`, in the panel's own coordinates.
pub fn action_at(view: &HighscoresView, cursor: [f32; 2]) -> Option<HighscoresAction> {
    let state = view.state.as_ref()?;
    let (category, profession) = (view.category(), view.profession());
    if let Some(index) = (0..CATEGORIES.len()).find(|index| category_tab(*index).contains_local(cursor)) {
        return Some(HighscoresAction::Show { category: index as u32, profession, page: 0 });
    }
    if let Some(index) = (0..PROFESSIONS.len()).find(|index| profession_chip(*index).contains_local(cursor)) {
        return Some(HighscoresAction::Show { category, profession: index as u32, page: 0 });
    }
    if OWN.contains_local(cursor) && state.own_rank > 0 {
        return Some(HighscoresAction::OwnPage { category, profession });
    }
    let last = state.pages.max(1) - 1;
    let page = (0..4).find(|index| pager_button(*index).contains_local(cursor)).map(|index| match index {
        0 => 0,
        1 => state.page.saturating_sub(1),
        2 => (state.page + 1).min(last),
        _ => last,
    })?;
    (page != state.page).then_some(HighscoresAction::Show { category, profession, page })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> HighscoresView {
        let mut view = HighscoresView { own_name: "Hero24".into(), own_mesh: 671_003, ..Default::default() };
        let names = ["Obispr", "Hidding", "Krankun", "Xatuba", "Sadysta", "Playzinho", "Taki", "Lowca", "Green Guy", "Duundu"];
        for (index, name) in names.iter().enumerate() {
            view.receive(HighscoreRow {
                index: index as u32,
                total: 13,
                kind: 0,
                rank: index as u32 + 21,
                value: 130 - index as u64,
                level: 130 - index as u32,
                rebirths: if index < 3 { 2 } else { 0 },
                class: [15, 25, 45, 135, 145][index % 5],
                mesh: 1003 + (index as u32 % 4 + 1) * 10_000 * 67,
                name: if index == 3 { "Hero24".into() } else { (*name).into() },
            });
        }
        for rank in 1..=3 {
            view.receive(HighscoreRow {
                index: 9 + rank,
                total: 13,
                kind: 1,
                rank,
                value: 140 - u64::from(rank),
                level: 140 - rank,
                rebirths: 2,
                class: 15,
                mesh: 671_003,
                name: ["Obispr", "Hidding", "Krankun"][rank as usize - 1].into(),
            });
        }
        view.state = Some(HighscoreState {
            category: 0,
            profession: 0,
            page: 2,
            pages: 82,
            own_rank: 24,
            own_value: 127,
            updated_at: crate::store_panel::now_secs() - 7 * 60,
            own_mesh: 671_003,
        });
        view
    }

    #[test]
    fn clicks_ask_for_pages_categories_and_the_own_page() {
        let view = view();
        assert_eq!(view.rows.len(), 10);
        assert_eq!(view.podium.len(), 3);
        let at = |rect: UiRect| [rect.x as f32 + 4.0, rect.y as f32 + 4.0];
        assert_eq!(action_at(&view, at(pager_button(2))), Some(HighscoresAction::Show { category: 0, profession: 0, page: 3 }));
        assert_eq!(action_at(&view, at(pager_button(3))), Some(HighscoresAction::Show { category: 0, profession: 0, page: 81 }));
        assert_eq!(action_at(&view, at(category_tab(1))), Some(HighscoresAction::Show { category: 1, profession: 0, page: 0 }));
        assert_eq!(action_at(&view, at(profession_chip(4))), Some(HighscoresAction::Show { category: 0, profession: 4, page: 0 }));
        assert_eq!(action_at(&view, at(OWN)), Some(HighscoresAction::OwnPage { category: 0, profession: 0 }));
    }

    /// `BOUNTY_PREVIEW=dir cargo test -p conquer-sandbox highscores_preview`
    #[test]
    fn highscores_preview() {
        let Ok(dir) = std::env::var("BOUNTY_PREVIEW") else { return };
        let mut assets = Assets::open(std::path::Path::new("R:/5065")).expect("assets");
        let mut content = RgbaImage::new(520, 420);
        draw(&mut content, &mut assets, Some(&view()));
        let mut image = crate::original_panels::framed_window(&mut assets, 520, 420).expect("frame");
        image::imageops::overlay(&mut image, &content, 0, 0);
        image.save(format!("{dir}/highscores.png")).expect("png");
    }
}
