//! The original 5065 bottom HUD, composed from the client's own artwork.
//!
//! `Control.Ani` binds the main dialog (`[Dialog4]`) to two 256px atlases and
//! the chat strip (`[Dialog5]`) to four more. The client assembles them into
//! the 1024x141 bar described by `GUI.ini` `[0-130]`; the offsets below were
//! measured against a screenshot of the original client and match it to the
//! pixel. Controls are drawn at their `GUI.ini` rectangles at native size:
//! the original never scales its HUD.

use crate::assets::{Assets, Result};
use crate::original_ui_layout::{ChatStripControl, EXP_RECT, MainHudButton};
use image::{Pixel, Rgba, RgbaImage, imageops};
use std::sync::OnceLock;

/// Atlas slice: source rectangle and its destination inside the HUD.
struct Slice {
    asset: &'static str,
    source: [u32; 4],
    dest: [i64; 2],
}

/// The slice holding the "Conquer" emblem, relettered "v0x".

const SLICES: [Slice; 8] = [
    // Orb with the four small buttons, F1-F4.
    Slice {
        asset: "data/main/mainDialog1.dds",
        source: [0, 128, 256, 128],
        dest: [0, 11],
    },
    // F5-F10.
    Slice {
        asset: "data/main/mainDialog1.dds",
        source: [0, 0, 256, 53],
        dest: [256, 86],
    },
    // Button grid and the left half of the "Skill" button.
    Slice {
        asset: "data/main/mainDialog2.dds",
        source: [0, 0, 256, 54],
        dest: [512, 85],
    },
    // The rest of "Skill" and the Conquer emblem.
    Slice {
        asset: "data/main/mainDialog2.dds",
        source: [0, 64, 256, 55],
        dest: [768, 84],
    },
    // Chat strip, drawn 2px above its GUI.ini origin like the original.
    Slice {
        asset: "data/main/DialogTalk1.dds",
        source: [0, 0, 256, 26],
        dest: [82, 68],
    },
    Slice {
        asset: "data/main/DialogTalk2.dds",
        source: [0, 0, 256, 26],
        dest: [338, 68],
    },
    Slice {
        asset: "data/main/DialogTalk3.dds",
        source: [0, 0, 256, 26],
        dest: [594, 68],
    },
    Slice {
        asset: "data/main/DialogTalk4.dds",
        source: [0, 0, 174, 26],
        dest: [850, 68],
    },
];

/// Normal and pressed frames for a control, from `Control.Ani`.
fn button_frames(button: MainHudButton) -> Option<[&'static str; 2]> {
    Some(match button {
        MainHudButton::Status => ["data/main/QueryBtn.dds", "data/main/QueryBtnClick.dds"],
        MainHudButton::Items => ["data/main/GoodBtn.dds", "data/main/GoodBtnClick.dds"],
        MainHudButton::Friend => ["data/main/SkillBtn.dds", "data/main/SkillBtnClick.dds"],
        MainHudButton::Capture => ["data/main/PkArre.dds", "data/main/PkArreClick.dds"],
        MainHudButton::Guild => ["data/main/TalkBtnB.dds", "data/main/TalkBtnBClick.dds"],
        MainHudButton::Action => ["data/main/FaceBtn.dds", "data/main/FaceBtnClick.dds"],
        MainHudButton::Trade => ["data/main/ChatBtn.dds", "data/main/ChatBtnClick.dds"],
        MainHudButton::Team => ["data/main/GroupBtn.dds", "data/main/GroupBtnClick.dds"],
        MainHudButton::Options => ["data/main/SetBtn.dds", "data/main/SetBtnClick.dds"],
        MainHudButton::Message => ["data/main/LevWordBtn.dds", "data/main/LevWordBtnClick.dds"],
        MainHudButton::Run => ["data/main/RunChk1.dds", "data/main/RunChk2.dds"],
        MainHudButton::NpcEquip => ["data/main/NpcEquip.dds", "data/main/NpcEquipClick.dds"],
        MainHudButton::ScreenMove => [
            "data/main/ScreenMoveChk1.dds",
            "data/main/ScreenMoveChk2.dds",
        ],
        MainHudButton::Map => ["data/main/MapChk2.dds", "data/main/MapChk1.dds"],
        // Painted into the atlas itself.
        MainHudButton::Skill => return None,
    })
}

/// `Control.Ani` art of the kill-mode button for PK, PEACE, TEAM and
/// CAPTURE: the button names the mode in force.
const KILL_MODE_FRAMES: [[&str; 2]; 4] = [
    ["data/main/PkFree.dds", "data/main/PkFreeClick.dds"],
    ["data/main/PkSafe.dds", "data/main/PkSafeClick.dds"],
    ["data/main/PkGroup.dds", "data/main/PkGroupClick.dds"],
    ["data/main/PkArre.dds", "data/main/PkArreClick.dds"],
];

fn chat_frames(control: ChatStripControl) -> Option<[&'static str; 2]> {
    Some(match control {
        ChatStripControl::List => [
            "data/main/TalkBtnList.dds",
            "data/main/TalkBtnListClick.dds",
        ],
        ChatStripControl::Add => [
            "data/main/TalkBtnAdd1.dds",
            "data/main/TalkBtnAdd1Click.dds",
        ],
        ChatStripControl::Talk => ["data/main/TalkBtnG.dds", "data/main/TalkBtnGClick.dds"],
        ChatStripControl::Emotion => ["data/main/TalkBtnAct.dds", "data/main/TalkBtnActClick.dds"],
        ChatStripControl::Send => [
            "data/main/TalkBtnSend.dds",
            "data/main/TalkBtnSendClick.dds",
        ],
        ChatStripControl::Clear => [
            "data/main/TalkBtnClear.dds",
            "data/main/TalkBtnClearClick.dds",
        ],
        ChatStripControl::Log => ["data/main/TalkBtnLog.dds", "data/main/TalkBtnLogClick.dds"],
        ChatStripControl::Name | ChatStripControl::Input => return None,
    })
}

const ORB_BACKGROUND: &str = "data/main/ProgressBk.dds";
const HP_FILL: &str = "data/main/ProgressHP.dds";
const MP_FILL: &str = "data/main/ProgressMP.dds";
const STAMINA_FILL: &str = "data/main/ProgressForce.dds";
/// Texture origins of the orb layers; the fills are clipped to their
/// `GUI.ini` rectangles `[130-1501]`/`[130-1502]`.
const HP_ORIGIN: [i64; 2] = [4, 54];
const HP_CLIP: [u32; 2] = [40, 86];
const MP_ORIGIN: [i64; 2] = [52, 54];
const MP_CLIP: [u32; 2] = [40, 86];
const STAMINA_ORIGIN: [i64; 2] = [44, 57];
/// The XP circle (`Control.Ani` `[Progress42]`): a gold ring around the orb
/// that fills with the XP points and lights up full.
const XP_FILL: &str = "data/main/ProgressPower.dds";
const XP_FULL: &str = "data/main/ProgressPowerH.dds";
/// Where the ring's texture goes so it lies in the frame's dark band: the
/// ring's centre (47, 45 in the texture) on the orb's (47, 93).
const XP_ORIGIN: [i64; 2] = [0, 48];

/// Decoded artwork, cropped and cached for the life of the process.
pub struct MainHudArt {
    base: RgbaImage,
    orb_background: RgbaImage,
    hp: RgbaImage,
    mp: RgbaImage,
    stamina: RgbaImage,
    xp: RgbaImage,
    xp_full: RgbaImage,
    buttons: Vec<(MainHudButton, [RgbaImage; 2])>,
    /// The Capture button's art for each kill mode, `shop::KILL_MODES` order.
    kill_modes: Vec<[RgbaImage; 2]>,
    chat: Vec<(ChatStripControl, [RgbaImage; 2])>,
}

fn visible(assets: &mut Assets, path: &str) -> Result<RgbaImage> {
    crate::original_ui::visible_part(&assets.image(path)?)
}

fn clipped(assets: &mut Assets, path: &str, size: [u32; 2]) -> Result<RgbaImage> {
    let source = assets.image(path)?;
    Ok(imageops::crop_imm(&source, 0, 0, size[0], size[1]).to_image())
}

impl MainHudArt {
    fn load(assets: &mut Assets) -> Result<Self> {
        // The orb frame and the bar artwork are static; flatten them once.
        let mut base = RgbaImage::new(1024, 141);
        for slice in SLICES {
            let source = assets.image(slice.asset)?;
            let [x, y, width, height] = slice.source;
            let part = imageops::crop_imm(&source, x, y, width, height).to_image();
            imageops::overlay(&mut base, &part, slice.dest[0], slice.dest[1]);
        }
        // Gravak's emblem over the original's, bottom right.
        crate::logo::draw_emblem(&mut base);
        let mut buttons = Vec::new();
        for button in MainHudButton::ALL {
            if let Some([normal, pressed]) = button_frames(button) {
                buttons.push((
                    button,
                    [visible(assets, normal)?, visible(assets, pressed)?],
                ));
            }
        }
        let kill_modes = KILL_MODE_FRAMES
            .iter()
            .map(|[normal, pressed]| Ok([visible(assets, normal)?, visible(assets, pressed)?]))
            .collect::<Result<Vec<_>>>()?;
        let mut chat = Vec::new();
        for control in ChatStripControl::ALL {
            if let Some([normal, pressed]) = chat_frames(control) {
                chat.push((
                    control,
                    [visible(assets, normal)?, visible(assets, pressed)?],
                ));
            }
        }
        Ok(Self {
            base,
            orb_background: assets.image(ORB_BACKGROUND)?,
            hp: clipped(assets, HP_FILL, HP_CLIP)?,
            mp: clipped(assets, MP_FILL, MP_CLIP)?,
            stamina: assets.image(STAMINA_FILL)?,
            xp: assets.image(XP_FILL)?,
            xp_full: assets.image(XP_FULL)?,
            buttons,
            kill_modes,
            chat,
        })
    }

    pub fn get(assets: &mut Assets) -> Result<&'static Self> {
        static ART: OnceLock<MainHudArt> = OnceLock::new();
        if let Some(art) = ART.get() {
            return Ok(art);
        }
        let art = Self::load(assets)?;
        Ok(ART.get_or_init(|| art))
    }
}

/// What the bar shows; everything else is static artwork.
pub struct MainHudState<'a> {
    pub health: u32,
    pub max_health: u32,
    pub mana: u32,
    pub max_mana: u32,
    pub stamina: u32,
    pub max_stamina: u32,
    /// XP points, 0-100, and whether the circle is lit.
    pub xp: u32,
    pub xp_ready: bool,
    pub experience: f32,
    pub hovered_button: Option<MainHudButton>,
    pub hovered_chat: Option<ChatStripControl>,
    pub chat_text: &'a str,
    pub chat_focused: bool,
    /// The kill mode in force, `shop::KILL_MODES` order.
    pub kill_mode: u8,
    /// Icon of the selected skill, painted over the "Skill" button.
    pub active_skill_icon: Option<RgbaImage>,
}

fn ratio(current: u32, maximum: u32) -> f32 {
    if maximum == 0 {
        0.0
    } else {
        (current as f32 / maximum as f32).clamp(0.0, 1.0)
    }
}

/// Draws `fill` as a vertical gauge that empties from the top, like the
/// original HP/MP orb halves.
fn draw_gauge(img: &mut RgbaImage, fill: &RgbaImage, origin: [i64; 2], fraction: f32) {
    let rows =
        (0..fill.height()).filter(|&y| (0..fill.width()).any(|x| fill.get_pixel(x, y)[3] > 0));
    let (Some(top), Some(bottom)) = (rows.clone().min(), rows.max()) else {
        return;
    };
    let span = (bottom + 1 - top) as f32;
    let first = bottom + 1 - (span * fraction).round() as u32;
    if first > bottom {
        return;
    }
    let part = imageops::crop_imm(fill, 0, first, fill.width(), bottom + 1 - first).to_image();
    imageops::overlay(img, &part, origin[0], origin[1] + i64::from(first));
}

fn blend(img: &mut RgbaImage, x: i64, y: i64, color: [u8; 4]) {
    if x >= 0 && y >= 0 && x < i64::from(img.width()) && y < i64::from(img.height()) {
        img.get_pixel_mut(x as u32, y as u32).blend(&Rgba(color));
    }
}

/// Paints the bar into `img` with its top-left corner at `origin`.
pub fn draw(
    img: &mut RgbaImage,
    art: &MainHudArt,
    origin: [i64; 2],
    state: &MainHudState,
    font: &fontdue::Font,
) {
    let at = |p: [i64; 2]| [origin[0] + p[0], origin[1] + p[1]];

    // The orb background and fills sit behind the semi-transparent glass of
    // the frame, which tints them exactly like the original.
    let [x, y] = at([0, 0]);
    imageops::overlay(img, &art.orb_background, x, y);
    let [x, y] = at(HP_ORIGIN);
    draw_gauge(img, &art.hp, [x, y], ratio(state.health, state.max_health));
    let [x, y] = at(MP_ORIGIN);
    draw_gauge(img, &art.mp, [x, y], ratio(state.mana, state.max_mana));
    let [x, y] = at(STAMINA_ORIGIN);
    draw_gauge(img, &art.stamina, [x, y], ratio(state.stamina, state.max_stamina));
    imageops::overlay(img, &art.base, origin[0], origin[1]);
    // The XP circle over the frame: filling from the bottom, lit when full.
    let [x, y] = at(XP_ORIGIN);
    if state.xp_ready {
        imageops::overlay(img, &art.xp_full, x, y);
    } else {
        draw_gauge(img, &art.xp, [x, y], ratio(state.xp, 100));
    }

    // Experience: a four-row gold gradient sampled from the original.
    const EXP_ROWS: [[u8; 4]; 4] = [
        [250, 204, 124, 255],
        [220, 163, 64, 255],
        [215, 163, 78, 255],
        [184, 144, 80, 255],
    ];
    let filled = (EXP_RECT.width * state.experience.clamp(0.0, 1.0)).round() as i64;
    for (row, color) in EXP_ROWS.into_iter().enumerate() {
        for column in 0..filled {
            blend(
                img,
                origin[0] + EXP_RECT.x as i64 + column,
                origin[1] + EXP_RECT.y as i64 + row as i64,
                color,
            );
        }
    }

    if let Some(icon) = &state.active_skill_icon {
        let rect = MainHudButton::Skill.local_rect();
        let icon = imageops::resize(
            icon,
            rect.width as u32 - 2,
            rect.height as u32 - 2,
            imageops::FilterType::Triangle,
        );
        let [x, y] = at([rect.x as i64 + 1, rect.y as i64 + 1]);
        imageops::overlay(img, &icon, x, y);
    }
    for (button, frames) in &art.buttons {
        let frames = match button {
            MainHudButton::Capture => art.kill_modes.get(usize::from(state.kill_mode)).unwrap_or(frames),
            _ => frames,
        };
        let rect = button.local_rect();
        let frame = &frames[usize::from(state.hovered_button == Some(*button))];
        let [x, y] = at([rect.x as i64, rect.y as i64]);
        imageops::overlay(img, frame, x, y);
    }
    for (control, frames) in &art.chat {
        let rect = control.local_rect();
        let frame = &frames[usize::from(state.hovered_chat == Some(*control))];
        let [x, y] = at([rect.x as i64, rect.y as i64]);
        imageops::overlay(img, frame, x, y);
    }

    let input = ChatStripControl::Input.local_rect();
    let typed = if state.chat_focused {
        format!("{}|", state.chat_text)
    } else {
        state.chat_text.to_owned()
    };
    let [x, y] = at([input.x as i64 + 3, input.y as i64 + 4]);
    text(
        img,
        font,
        &typed,
        12.0,
        [x as i32, y as i32],
        [255, 255, 255, 255],
        Some(input.width as i32 - 6),
    );
}

/// `ani/Magic.Ani`: skill id -> (icon texture, whether it is an XP skill).
fn magic_catalog(assets: &mut Assets) -> &'static std::collections::HashMap<u32, (String, bool)> {
    static CATALOG: OnceLock<std::collections::HashMap<u32, (String, bool)>> = OnceLock::new();
    if let Some(catalog) = CATALOG.get() {
        return catalog;
    }
    let mut catalog = std::collections::HashMap::new();
    if let Ok(bytes) = assets.read("ani/Magic.Ani") {
        let text = String::from_utf8_lossy(&bytes);
        let mut current = None;
        for line in text.lines().map(str::trim) {
            if let Some(name) = line.strip_prefix('[').and_then(|line| line.strip_suffix(']')) {
                current = name
                    .strip_prefix("MagicSkillType")
                    .map(|id| (id, false))
                    .or_else(|| name.strip_prefix("XpSkillType").map(|id| (id, true)))
                    .and_then(|(id, xp)| id.parse::<u32>().ok().map(|id| (id, xp)));
                continue;
            }
            if let (Some((id, xp)), Some(path)) = (current, line.strip_prefix("Frame0=")) {
                // XP entries win: they decide which row a skill belongs to.
                let entry = catalog.entry(id).or_insert((path.trim().to_owned(), xp));
                if xp {
                    *entry = (path.trim().to_owned(), true);
                }
                current = None;
            }
        }
    }
    CATALOG.get_or_init(|| catalog)
}

/// Original skill icon (with its gold-cornered frame), cropped to the art.
pub fn skill_icon(assets: &mut Assets, skill_id: u32) -> Option<RgbaImage> {
    let path = magic_catalog(assets)
        .get(&skill_id)
        .map(|(path, _)| path.clone())
        .unwrap_or_else(|| crate::action_bar::skill_icon(skill_id));
    let image = assets.image(&path).ok()?;
    crate::original_ui::visible_part(&image).ok()
}

pub fn is_xp_skill(assets: &mut Assets, skill_id: u32) -> bool {
    magic_catalog(assets)
        .get(&skill_id)
        .is_some_and(|(_, xp)| *xp)
}

/// Sans-serif UI font close to the original client's Tahoma text.
pub fn font() -> Result<&'static fontdue::Font> {
    static FONT: OnceLock<fontdue::Font> = OnceLock::new();
    if let Some(font) = FONT.get() {
        return Ok(font);
    }
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let id = db
        .query(&fontdb::Query {
            families: &[
                fontdb::Family::Name("Tahoma"),
                fontdb::Family::Name("Verdana"),
                fontdb::Family::Name("Arial"),
                fontdb::Family::SansSerif,
            ],
            ..Default::default()
        })
        .ok_or("No UI font available")?;
    let value = db
        .with_face_data(id, |bytes, index| {
            fontdue::Font::from_bytes(
                bytes,
                fontdue::FontSettings {
                    collection_index: index,
                    ..Default::default()
                },
            )
        })
        .ok_or("UI font unavailable")??;
    Ok(FONT.get_or_init(|| value))
}

/// `ini/Font.ini` asks for "Courier New 12", but the client creates it with
/// the GB2312 charset, so Windows substitutes SimSun: 6x12 ASCII glyphs,
/// drawn by GDI without antialiasing.
pub fn client_font() -> Result<&'static fontdue::Font> {
    static FONT: OnceLock<fontdue::Font> = OnceLock::new();
    if let Some(font) = FONT.get() {
        return Ok(font);
    }
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let id = db
        .query(&fontdb::Query {
            families: &[
                fontdb::Family::Name("SimSun"),
                fontdb::Family::Name("NSimSun"),
                fontdb::Family::Name("Courier New"),
                fontdb::Family::Monospace,
            ],
            ..Default::default()
        })
        .ok_or("No client font available")?;
    let value = db
        .with_face_data(id, |bytes, index| {
            fontdue::Font::from_bytes(
                bytes,
                fontdue::FontSettings {
                    collection_index: index,
                    ..Default::default()
                },
            )
        })
        .ok_or("Client font unavailable")??;
    Ok(FONT.get_or_init(|| value))
}

/// Crisp serif for gold text-only buttons that have no original art.
pub fn label_font() -> Result<&'static fontdue::Font> {
    static FONT: OnceLock<fontdue::Font> = OnceLock::new();
    if let Some(font) = FONT.get() {
        return Ok(font);
    }
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let id = db
        .query(&fontdb::Query {
            families: &[
                fontdb::Family::Name("Times New Roman"),
                fontdb::Family::Serif,
            ],
            ..Default::default()
        })
        .ok_or("No label font available")?;
    let value = db
        .with_face_data(id, |bytes, index| {
            fontdue::Font::from_bytes(
                bytes,
                fontdue::FontSettings {
                    collection_index: index,
                    ..Default::default()
                },
            )
        })
        .ok_or("Label font unavailable")??;
    Ok(FONT.get_or_init(|| value))
}

/// Italic sans for the large glowing numbers (battle power, KO count).
pub fn italic_font() -> Result<&'static fontdue::Font> {
    static FONT: OnceLock<fontdue::Font> = OnceLock::new();
    if let Some(font) = FONT.get() {
        return Ok(font);
    }
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let id = db
        .query(&fontdb::Query {
            families: &[
                fontdb::Family::Name("Arial"),
                fontdb::Family::Name("Tahoma"),
                fontdb::Family::SansSerif,
            ],
            style: fontdb::Style::Italic,
            ..Default::default()
        })
        .ok_or("No italic font available")?;
    let value = db
        .with_face_data(id, |bytes, index| {
            fontdue::Font::from_bytes(
                bytes,
                fontdue::FontSettings {
                    collection_index: index,
                    ..Default::default()
                },
            )
        })
        .ok_or("Italic font unavailable")??;
    Ok(FONT.get_or_init(|| value))
}

/// White text with a soft coloured glow, as the original draws its large
/// numbers (the battle power in the Character orb, the KO counter).
pub fn glow_text(
    img: &mut RgbaImage,
    font: &fontdue::Font,
    value: &str,
    size: f32,
    top_left: [i32; 2],
    glow: [u8; 3],
) {
    for (radius, alpha) in [(2, 70), (1, 200)] {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx != 0 || dy != 0 {
                    glyphs(
                        img,
                        font,
                        value,
                        size,
                        [top_left[0] + dx, top_left[1] + dy],
                        [glow[0], glow[1], glow[2], alpha],
                    );
                }
            }
        }
    }
    glyphs(img, font, value, size, top_left, [255, 255, 255, 255]);
}

/// Antialiased text in one colour, without outline.
fn glyphs(
    img: &mut RgbaImage,
    font: &fontdue::Font,
    value: &str,
    size: f32,
    top_left: [i32; 2],
    colour: [u8; 4],
) {
    let ascent = font
        .horizontal_line_metrics(size)
        .map_or(size * 0.8, |metrics| metrics.ascent);
    let baseline = top_left[1] + ascent.round() as i32;
    let mut cursor = top_left[0] as f32;
    for ch in value.chars() {
        let (metrics, bitmap) = font.rasterize(ch, size);
        for row in 0..metrics.height {
            for column in 0..metrics.width {
                let alpha = bitmap[row * metrics.width + column];
                if alpha == 0 {
                    continue;
                }
                let x = cursor.round() as i32 + metrics.xmin + column as i32;
                let y = baseline - metrics.ymin - metrics.height as i32 + row as i32;
                let a = (u16::from(alpha) * u16::from(colour[3]) / 255) as u8;
                blend(img, x.into(), y.into(), [colour[0], colour[1], colour[2], a]);
            }
        }
        cursor += metrics.advance_width;
    }
}

pub fn text_width(font: &fontdue::Font, value: &str, size: f32) -> i32 {
    value
        .chars()
        .map(|c| font.metrics(c, size).advance_width)
        .sum::<f32>()
        .ceil() as i32
}

/// Draws `value` with the one-pixel dark outline the original uses for text
/// over the world. `top_left` is the top of the line box; text past
/// `max_width` is clipped. Returns the advance width.
pub fn text(
    img: &mut RgbaImage,
    font: &fontdue::Font,
    value: &str,
    size: f32,
    top_left: [i32; 2],
    color: [u8; 4],
    max_width: Option<i32>,
) -> i32 {
    draw_text(img, font, value, size, top_left, color, max_width, false)
}

/// Like [`text`], but without antialiasing, the way GDI draws the client
/// font in the original UI.
pub fn crisp_text(
    img: &mut RgbaImage,
    font: &fontdue::Font,
    value: &str,
    size: f32,
    top_left: [i32; 2],
    color: [u8; 4],
) -> i32 {
    draw_text(img, font, value, size, top_left, color, None, true)
}

fn draw_text(
    img: &mut RgbaImage,
    font: &fontdue::Font,
    value: &str,
    size: f32,
    top_left: [i32; 2],
    color: [u8; 4],
    max_width: Option<i32>,
    crisp: bool,
) -> i32 {
    let ascent = font
        .horizontal_line_metrics(size)
        .map_or(size * 0.8, |metrics| metrics.ascent);
    let baseline = top_left[1] + ascent.round() as i32;
    let limit = max_width.map(|width| top_left[0] + width);
    let mut cursor = top_left[0] as f32;
    for ch in value.chars() {
        let (metrics, mut bitmap) = font.rasterize(ch, size);
        if crisp {
            for alpha in &mut bitmap {
                *alpha = if *alpha >= 40 { 255 } else { 0 };
            }
        }
        for row in 0..metrics.height {
            for column in 0..metrics.width {
                let alpha = bitmap[row * metrics.width + column];
                if alpha == 0 {
                    continue;
                }
                let x = cursor.round() as i32 + metrics.xmin + column as i32;
                let y = baseline - metrics.ymin - metrics.height as i32 + row as i32;
                if limit.is_some_and(|limit| x >= limit) {
                    continue;
                }
                let shadow = [0, 0, 0, (u16::from(alpha) * 200 / 255) as u8];
                let offsets: &[[i32; 2]] = if crisp {
                    &[[1, 1]]
                } else {
                    &[[1, 0], [-1, 0], [0, 1], [0, -1], [1, 1]]
                };
                for &[dx, dy] in offsets {
                    blend(img, i64::from(x + dx), i64::from(y + dy), shadow);
                }
            }
        }
        for row in 0..metrics.height {
            for column in 0..metrics.width {
                let alpha = bitmap[row * metrics.width + column];
                let x = cursor.round() as i32 + metrics.xmin + column as i32;
                let y = baseline - metrics.ymin - metrics.height as i32 + row as i32;
                if alpha == 0 || limit.is_some_and(|limit| x >= limit) {
                    continue;
                }
                let a = (u16::from(alpha) * u16::from(color[3]) / 255) as u8;
                blend(
                    img,
                    i64::from(x),
                    i64::from(y),
                    [color[0], color[1], color[2], a],
                );
            }
        }
        cursor += metrics.advance_width;
    }
    (cursor - top_left[0] as f32).ceil() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gauges_empty_from_the_top() {
        let fill = RgbaImage::from_pixel(4, 10, Rgba([255, 0, 0, 255]));
        let mut half = RgbaImage::new(4, 10);
        draw_gauge(&mut half, &fill, [0, 0], 0.5);
        assert_eq!(half.get_pixel(0, 4)[3], 0);
        assert_eq!(half.get_pixel(0, 5)[3], 255);
        let mut empty = RgbaImage::new(4, 10);
        draw_gauge(&mut empty, &fill, [0, 0], 0.0);
        assert!(empty.pixels().all(|pixel| pixel[3] == 0));
        assert_eq!(ratio(5, 0), 0.0);
    }
}
