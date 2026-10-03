//! Social and utility windows (Friend, Team, Guild, Trade, Options and
//! Message) rebuilt from their original 5065 dialogs.
//!
//! Each window is described by a frame (`Control.Ani` dialog art, size and
//! `GUI.ini` anchor) and by a list of controls computed from the live state.
//! The same list is used to paint the window and to hit-test clicks, so the
//! two cannot drift apart.

use crate::assets::{Assets, Result};
use crate::network::TradeAmount;
use crate::shop::{
    FriendsPanelAction, GuildPanelAction, QuickPanel, TeamPanelAction, TradePanelAction,
};
use image::{Pixel, Rgba, RgbaImage, imageops};

#[derive(Clone, Copy)]
enum Anchor {
    /// Left edge, vertically centred (the original `x=0, y=65` dialogs).
    Left,
    /// Top centre, below the network status line.
    TopCenter,
    Center,
}

struct Frame {
    art: &'static str,
    size: [u32; 2],
    anchor: Anchor,
    /// Small top-right close button and, optionally, a bottom "Close".
    close: &'static [[i32; 4]],
}

fn frame(panel: QuickPanel) -> Option<Frame> {
    Some(match panel {
        // [0-160] plus [160-1681] close.
        QuickPanel::Friends => Frame {
            art: "data/main/GeneralBG.dds",
            size: [208, 375],
            anchor: Anchor::Left,
            close: &[[175, 10, 15, 15]],
        },
        // [0-141] plus [141-1642] close.
        QuickPanel::Team => Frame {
            art: "data/main/DialogGroup.dds",
            size: [290, 114],
            anchor: Anchor::TopCenter,
            close: &[[263, 10, 15, 15]],
        },
        // [0-133] plus [133-1510]/[133-1511].
        QuickPanel::Guild => Frame {
            art: "data/main/DialogQuery_Faction.dds",
            size: [250, 392],
            anchor: Anchor::Left,
            close: &[[210, 23, 15, 15], [144, 350, 63, 21]],
        },
        // [0-137] plus [137-1138].
        QuickPanel::Trade => Frame {
            art: "data/main/DialogTrade.dds",
            size: [335, 297],
            anchor: Anchor::Left,
            close: &[[303, 8, 15, 15]],
        },
        // [0-138] plus [138-1535]/[138-1531].
        QuickPanel::Settings => Frame {
            art: "data/main/DialogSetup.dds",
            size: [370, 294],
            anchor: Anchor::Center,
            close: &[[340, 23, 15, 15], [244, 261, 62, 20]],
        },
        // [0-232] plus [232-1281]/[232-1283].
        QuickPanel::Mail => Frame {
            art: "data/main/DialogMsgSend.dds",
            size: [396, 261],
            anchor: Anchor::Center,
            close: &[[372, 20, 16, 16], [290, 230, 64, 24]],
        },
        // The original VIP information art (`VipInfoBG.dds`).
        QuickPanel::Vip => Frame {
            art: "data/main/VipInfoBG.dds",
            size: [330, 200],
            anchor: Anchor::Center,
            close: &[[304, 8, 16, 16]],
        },
        _ => return None,
    })
}

/// The Renew/Get VIP button on the VIP window's bottom band.
const VIP_RENEW: [i32; 4] = [186, 167, 124, 22];

/// Whether the cursor is on the VIP window's Renew/Get VIP button.
pub fn vip_renew_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    local(QuickPanel::Vip, viewport, cursor).is_some_and(|point| inside(VIP_RENEW, point))
}

/// The VIP benefits, as the window lists them.
pub const VIP_PERKS: [&str; 4] = [
    "+10% monster experience",
    "Auto-loot silver and rares",
    "Free Conductress rides",
    "VIP title over your name",
];

/// The VIP window's contents over its art.
fn draw_vip(image: &mut RgbaImage, assets: &mut Assets, vip_until: u64) -> Result<()> {
    crate::original_ui::overlay_asset(image, assets, "data/main/CloseBtnS.dds", 304, 8)?;
    // The title in the portrait box.
    let mut tag = crate::ornate::vip_tag(1.2)?;
    let active = vip_until
        > std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs());
    if !active {
        // Without VIP the title is shown in faded grey.
        for pixel in tag.pixels_mut() {
            let grey = (u16::from(pixel.0[0]) + u16::from(pixel.0[1]) + u16::from(pixel.0[2])) / 3;
            pixel.0 = [grey as u8, grey as u8, grey as u8, (u16::from(pixel.0[3]) * 3 / 5) as u8];
        }
    }
    let box_centre = [71_i64, 71_i64];
    imageops::overlay(
        image,
        &tag,
        box_centre[0] - i64::from(tag.width()) / 2,
        box_centre[1] - i64::from(tag.height()) / 2,
    );
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    let left = crate::store_panel::vip_left(vip_until, now);
    label_centered(image, "VIP Rights", 16.0, 227, 20, LABEL_GOLD);
    match &left {
        Some(left) => label_centered(image, &format!("Active - {left} left"), 13.0, 227, 42, [140, 230, 140, 255]),
        None => label_centered(image, "Not active", 13.0, 227, 42, GREY),
    }
    for (index, perk) in VIP_PERKS.iter().enumerate() {
        let y = 68 + index as i32 * 21;
        // A small gold diamond as the bullet.
        for dy in -3_i32..=3 {
            for dx in -(3 - dy.abs())..=(3 - dy.abs()) {
                image.put_pixel((142 + dx) as u32, (y + 7 + dy) as u32, Rgba([236, 194, 86, 255]));
            }
        }
        let colour = if left.is_some() { WHITE } else { GREY };
        text(image, perk, 152, y, colour, 160);
    }
    let [x, y, width, height] = VIP_RENEW;
    crate::components::draw_button(
        image,
        &*crate::labels::font()?,
        crate::ui::UiRect::new(x, y, width, height),
        if left.is_some() { "Renew VIP" } else { "Get VIP" },
        crate::components::WidgetState::Normal,
    );
    Ok(())
}

/// Whether `panel` uses an original dialog (the rest keep the generic frame).
pub fn is_original(panel: QuickPanel) -> bool {
    frame(panel).is_some()
}

pub fn position(panel: QuickPanel, viewport: [f32; 2]) -> Option<[f32; 2]> {
    let frame = frame(panel)?;
    let [width, height] = [frame.size[0] as f32, frame.size[1] as f32];
    Some(match frame.anchor {
        Anchor::Left => [0.0, ((viewport[1] - height) * 0.5).max(8.0)],
        Anchor::TopCenter => [((viewport[0] - width) * 0.5).max(0.0), 52.0],
        Anchor::Center => [
            ((viewport[0] - width) * 0.5).max(0.0),
            ((viewport[1] - height) * 0.5).max(8.0),
        ],
    })
}

fn local(panel: QuickPanel, viewport: [f32; 2], cursor: [f32; 2]) -> Option<[i32; 2]> {
    let [left, top] = position(panel, viewport)?;
    Some([
        (cursor[0] - left).floor() as i32,
        (cursor[1] - top).floor() as i32,
    ])
}

fn inside(rect: [i32; 4], point: [i32; 2]) -> bool {
    point[0] >= rect[0]
        && point[0] < rect[0] + rect[2]
        && point[1] >= rect[1]
        && point[1] < rect[1] + rect[3]
}

pub fn panel_at(panel: QuickPanel, viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    let (Some(point), Some(frame)) = (local(panel, viewport, cursor), frame(panel)) else {
        return false;
    };
    inside([0, 0, frame.size[0] as i32, frame.size[1] as i32], point)
}

pub fn close_at(panel: QuickPanel, viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    let (Some(point), Some(frame)) = (local(panel, viewport, cursor), frame(panel)) else {
        return false;
    };
    frame.close.iter().any(|&rect| inside(rect, point))
}

/// What a control shows.
enum Face {
    /// Original texture drawn at the control origin.
    Art(&'static str),
    /// Gold serif label, like the text-only buttons of the original
    /// (`GroupBtnCreate`, `GroupBtnJoin`, ...), for actions without art.
    Label(&'static str),
    /// Two-state check box texture.
    Check(&'static str, &'static str, bool),
    /// A small framed button with a label, lit when selected, for options
    /// the original window has no art for.
    Button(&'static str, bool),
    /// A check box drawn by the client, with its label on the right.
    Toggle(&'static str, bool),
}

struct Control<A> {
    rect: [i32; 4],
    face: Face,
    action: Option<A>,
}

fn control<A>(rect: [i32; 4], face: Face, action: Option<A>) -> Control<A> {
    Control { rect, face, action }
}

fn hit<A: Copy>(controls: &[Control<A>], point: [i32; 2]) -> Option<A> {
    controls
        .iter()
        .find(|control| control.action.is_some() && inside(control.rect, point))
        .and_then(|control| control.action)
}

fn paint<A>(image: &mut RgbaImage, assets: &mut Assets, controls: &[Control<A>]) -> Result<()> {
    for control in controls {
        let [x, y, width, height] = control.rect;
        match control.face {
            Face::Art(path) => {
                crate::original_ui::overlay_asset(image, assets, path, x.into(), y.into())?
            }
            Face::Check(off, on, checked) => crate::original_ui::overlay_asset(
                image,
                assets,
                if checked { on } else { off },
                x.into(),
                y.into(),
            )?,
            Face::Label(label) => {
                let font = crate::main_hud::label_font()?;
                let text_width = crate::main_hud::text_width(font, label, 13.0);
                crate::main_hud::crisp_text(
                    image,
                    font,
                    label,
                    13.0,
                    [x + (width - text_width) / 2, y + (height - 15) / 2],
                    [232, 206, 148, 255],
                );
            }
            Face::Button(label, selected) => {
                fill(image, control.rect, [6, 18, 30, 210]);
                if selected {
                    highlight(image, control.rect);
                } else {
                    outline(image, control.rect, [120, 100, 62, 255]);
                }
                let font = crate::main_hud::label_font()?;
                let text_width = crate::main_hud::text_width(font, label, 13.0);
                crate::main_hud::crisp_text(
                    image,
                    font,
                    label,
                    13.0,
                    [x + (width - text_width) / 2, y + (height - 15) / 2],
                    if selected { [255, 232, 160, 255] } else { [200, 180, 130, 255] },
                );
            }
            Face::Toggle(label, checked) => {
                let box_rect = [x, y + (height - 12) / 2, 12, 12];
                fill(image, box_rect, [6, 18, 30, 200]);
                outline(image, box_rect, [196, 170, 100, 255]);
                if checked {
                    fill(image, [box_rect[0] + 3, box_rect[1] + 3, 6, 6], [255, 215, 108, 255]);
                }
                let font = crate::main_hud::label_font()?;
                crate::main_hud::crisp_text(
                    image,
                    font,
                    label,
                    13.0,
                    [x + 18, y + (height - 15) / 2],
                    [232, 206, 148, 255],
                );
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Friend ([0-160])

const FRIEND_LIST: [i32; 4] = [22, 40, 150, 250];
const FRIEND_ROW: i32 = 16;
const FRIEND_ROWS: usize = 15;

/// Rows of the friend list: friends first, then the ignore list.
fn friend_rows(friends: usize, ignores: usize) -> Vec<FriendRow> {
    (0..friends)
        .map(FriendRow::Friend)
        .chain((0..ignores).map(FriendRow::Ignore))
        .take(FRIEND_ROWS)
        .collect()
}

#[derive(Clone, Copy)]
enum FriendRow {
    Friend(usize),
    Ignore(usize),
}

fn friend_row_rect(row: usize) -> [i32; 4] {
    [
        FRIEND_LIST[0],
        FRIEND_LIST[1] + row as i32 * FRIEND_ROW,
        FRIEND_LIST[2] - 20,
        FRIEND_ROW,
    ]
}

fn friend_remove_rect(row: usize) -> [i32; 4] {
    let [x, y, width, _] = friend_row_rect(row);
    [x + width - 12, y, 12, FRIEND_ROW]
}

/// "Make Friends" and "Enemy" are always live: with no player selected they
/// wait for the next player clicked. A received request adds Accept/Refuse.
fn friend_controls(pending_request: bool) -> Vec<Control<FriendsPanelAction>> {
    let mut controls = vec![
        control(
            [70, 10, 80, 17],
            Face::Art("data/main/DialogFriend.dds"),
            None,
        ),
        control(
            [175, 10, 15, 15],
            Face::Art("data/main/CloseBtnS.dds"),
            None,
        ),
        control(
            [156, 40, 16, 16],
            Face::Art("data/main/FriendBtnUp.dds"),
            None,
        ),
        control(
            [156, 271, 16, 16],
            Face::Art("data/main/FriendBtnDown.dds"),
            None,
        ),
        // [160-1679] "Make Friends" and [160-1682] "Enemy".
        control(
            [70, 308, 63, 20],
            Face::Art("data/main/FriendBtnAdd.dds"),
            Some(FriendsPanelAction::AddFriend),
        ),
        control(
            [80, 340, 43, 20],
            Face::Art("data/interface/Style01/Teacher/but-Enemy1.dds"),
            Some(FriendsPanelAction::AddIgnore),
        ),
    ];
    if pending_request {
        controls.extend([
            control(
                [140, 308, 52, 20],
                Face::Label("Accept"),
                Some(FriendsPanelAction::AcceptRequest),
            ),
            control(
                [140, 340, 52, 20],
                Face::Label("Refuse"),
                Some(FriendsPanelAction::RefuseRequest),
            ),
        ]);
    }
    controls
}

pub fn friends_action_at(
    viewport: [f32; 2],
    cursor: [f32; 2],
    friends: usize,
    ignores: usize,
    pending_request: bool,
) -> Option<FriendsPanelAction> {
    let point = local(QuickPanel::Friends, viewport, cursor)?;
    if let Some(action) = hit(&friend_controls(pending_request), point) {
        return Some(action);
    }
    for (row, entry) in friend_rows(friends, ignores).into_iter().enumerate() {
        let remove = inside(friend_remove_rect(row), point);
        if !inside(friend_row_rect(row), point) {
            continue;
        }
        return Some(match (entry, remove) {
            (FriendRow::Friend(index), true) => FriendsPanelAction::RemoveFriend(index),
            (FriendRow::Friend(index), false) => FriendsPanelAction::Whisper(index),
            (FriendRow::Ignore(index), _) => FriendsPanelAction::RemoveIgnore(index),
        });
    }
    None
}

// ---------------------------------------------------------------------------
// Team ([0-141])

const TEAM_PORTRAITS: [i32; 5] = [17, 65, 113, 162, 211];

pub struct TeamState {
    pub has_team: bool,
    pub pending_invite: bool,
    pub pending_join_request: bool,
    pub join_enabled: bool,
}

fn team_controls(state: &TeamState) -> Vec<Control<TeamPanelAction>> {
    use TeamPanelAction as A;
    let mut controls = vec![
        control(
            [263, 10, 15, 15],
            Face::Art("data/main/CloseBtnS.dds"),
            None,
        ),
        control([223, 60, 43, 17], Face::Label("Help"), None),
    ];
    // [141-1640], [141-1641], [141-1648], [141-1649] button row.
    if state.has_team {
        controls.extend([
            control(
                [13, 60, 43, 17],
                Face::Art("data/main/GroupBtnInvite.dds"),
                Some(A::Invite),
            ),
            control(
                [68, 60, 43, 17],
                Face::Art("data/main/GroupBtnOut.dds"),
                Some(A::Leave),
            ),
            control(
                [119, 60, 43, 17],
                Face::Art("data/main/GroupBtnUnlay.dds"),
                Some(A::Dismiss),
            ),
            control(
                [170, 60, 43, 17],
                Face::Art("data/main/GroupBtnKickout.dds"),
                Some(A::Kick),
            ),
            // [141-2019] check box: forbid new members.
            control(
                [20, 85, 57, 20],
                Face::Check(
                    "data/main/GroupChkStop1.dds",
                    "data/main/GroupChkStop2.dds",
                    !state.join_enabled,
                ),
                Some(A::ToggleJoin),
            ),
            control([90, 85, 60, 20], Face::Label("Leader"), Some(A::Leader)),
        ]);
        if state.pending_join_request {
            controls.push(control(
                [160, 85, 60, 20],
                Face::Label("Accept"),
                Some(A::AcceptJoin),
            ));
        }
    } else {
        controls.extend([
            control(
                [13, 60, 43, 17],
                Face::Art("data/main/GroupBtnCreate.dds"),
                Some(A::Create),
            ),
            control(
                [68, 60, 43, 17],
                Face::Art("data/main/GroupBtnJoin.dds"),
                Some(if state.pending_invite {
                    A::AcceptInvite
                } else {
                    A::RequestJoin
                }),
            ),
        ]);
    }
    controls
}

pub fn team_action_at(
    viewport: [f32; 2],
    cursor: [f32; 2],
    state: &TeamState,
) -> Option<TeamPanelAction> {
    hit(
        &team_controls(state),
        local(QuickPanel::Team, viewport, cursor)?,
    )
}

/// Index of the portrait under the cursor ([141-1643..1647]).
pub fn team_portrait_at(viewport: [f32; 2], cursor: [f32; 2]) -> Option<usize> {
    let point = local(QuickPanel::Team, viewport, cursor)?;
    TEAM_PORTRAITS
        .iter()
        .position(|&x| inside([x, 10, 47, 47], point))
}

// ---------------------------------------------------------------------------
// Guild ([0-133])

const GUILD_MEMBER_TOP: i32 = 150;
const GUILD_MEMBER_ROW: i32 = 16;
const GUILD_MEMBER_ROWS: usize = 6;

fn guild_controls(has_guild: bool, pending_invite: bool) -> Vec<Control<GuildPanelAction>> {
    use GuildPanelAction as A;
    let mut controls = vec![
        control(
            [210, 23, 15, 15],
            Face::Art("data/main/CloseBtnS.dds"),
            None,
        ),
        control([56, 350, 63, 21], Face::Art("data/main/HelpBtn2.dds"), None),
        control(
            [144, 350, 63, 21],
            Face::Art("data/main/CloseBtn2.dds"),
            None,
        ),
    ];
    if has_guild {
        controls.extend([
            // [133-1048]/[133-1049] and [133-1043]/[133-1046] button rows.
            control(
                [46, 293, 64, 22],
                Face::Art("data/main/FactionBtnKick.dds"),
                Some(A::Kick),
            ),
            control([134, 293, 64, 22], Face::Label("Invite"), Some(A::Invite)),
            control(
                [46, 320, 64, 22],
                Face::Art("data/main/FactionBtnOut.dds"),
                Some(A::Leave),
            ),
            control(
                [134, 320, 64, 22],
                Face::Label("Warehouse"),
                Some(A::Warehouse),
            ),
        ]);
    } else if pending_invite {
        controls.push(control(
            [134, 320, 64, 22],
            Face::Art("data/main/FactionBtnJoin.dds"),
            Some(A::AcceptInvite),
        ));
    }
    controls
}

pub fn guild_action_at(
    viewport: [f32; 2],
    cursor: [f32; 2],
    has_guild: bool,
    pending_invite: bool,
) -> Option<GuildPanelAction> {
    hit(
        &guild_controls(has_guild, pending_invite),
        local(QuickPanel::Guild, viewport, cursor)?,
    )
}

pub fn guild_member_at(viewport: [f32; 2], cursor: [f32; 2]) -> Option<usize> {
    let point = local(QuickPanel::Guild, viewport, cursor)?;
    (0..GUILD_MEMBER_ROWS).find(|&row| {
        inside(
            [
                30,
                GUILD_MEMBER_TOP + row as i32 * GUILD_MEMBER_ROW,
                190,
                GUILD_MEMBER_ROW,
            ],
            point,
        )
    })
}

// ---------------------------------------------------------------------------
// Trade ([0-137])

const TRADE_PEER_GRID: [i32; 2] = [21, 57];
const TRADE_OWN_GRID: [i32; 2] = [21, 168];
const TRADE_SLOT_STEP: i32 = 43;
const TRADE_SLOT: i32 = 40;
pub const TRADE_SLOTS: usize = 6;
const TRADE_OWN_SILVER: [i32; 4] = [211, 187, 88, 16];
const TRADE_OWN_CPS: [i32; 4] = [211, 210, 88, 16];

fn trade_slot_rect(grid: [i32; 2], index: usize) -> [i32; 4] {
    [
        grid[0] + (index % 3) as i32 * TRADE_SLOT_STEP,
        grid[1] + (index / 3) as i32 * TRADE_SLOT_STEP,
        TRADE_SLOT,
        TRADE_SLOT,
    ]
}

fn trade_controls(
    active: bool,
    pending_request: bool,
    own_items: usize,
) -> Vec<Control<TradePanelAction>> {
    use TradePanelAction as A;
    let mut controls = vec![
        control([303, 8, 15, 15], Face::Art("data/main/CloseBtnS.dds"), None),
        control([24, 261, 46, 20], Face::Art("data/main/HelpBtn2.dds"), None),
    ];
    if active {
        for index in 0..TRADE_SLOTS {
            controls.push(control(
                trade_slot_rect(TRADE_OWN_GRID, index),
                Face::Label(""),
                Some(if index < own_items {
                    A::RemoveOwnItem(index)
                } else {
                    A::AddItem
                }),
            ));
        }
        controls.extend([
            // [137-1570] own silver and [137-1573] own CPs. The typed amounts
            // are sent on Enter, when the box loses focus and on Agree, so the
            // original "confirm amount" button ([137-1145]) is left out.
            control(
                TRADE_OWN_SILVER,
                Face::Label(""),
                Some(A::EditAmount(TradeAmount::Silver)),
            ),
            control(
                TRADE_OWN_CPS,
                Face::Label(""),
                Some(A::EditAmount(TradeAmount::Cps)),
            ),
            // [137-1139] agree and [137-1571] cancel.
            control([112, 261, 62, 20], Face::Label("Agree"), Some(A::Confirm)),
            control([222, 261, 46, 20], Face::Label("Cancel"), Some(A::Cancel)),
        ]);
    } else if pending_request {
        controls.extend([
            control(
                [112, 261, 62, 20],
                Face::Label("Accept"),
                Some(A::AcceptRequest),
            ),
            control([222, 261, 46, 20], Face::Label("Refuse"), Some(A::Cancel)),
        ]);
    } else {
        // With no player selected, Request waits for the next one clicked.
        controls.push(control(
            [112, 261, 62, 20],
            Face::Label("Request"),
            Some(A::Request),
        ));
    }
    controls
}

pub fn trade_action_at(
    viewport: [f32; 2],
    cursor: [f32; 2],
    trade: &crate::network::TradeView,
) -> Option<TradePanelAction> {
    hit(
        &trade_controls(
            trade.active,
            trade.pending_request.is_some(),
            trade.own_items.len(),
        ),
        local(QuickPanel::Trade, viewport, cursor)?,
    )
}

// ---------------------------------------------------------------------------
// Options ([0-138]) and Message ([0-232]) have no client-side behaviour yet;
// they show the original controls only.

/// What a click in the Options window asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsAction {
    /// A volume, 0..=100, from where the bar was clicked.
    Music(u8),
    Sound(u8),
    /// An index into `hud::RESOLUTIONS`.
    Resolution(u8),
    ToggleNames,
    ToggleCounter,
    /// Full (`true`) or Simple shadows.
    Shadows(bool),
    Delete,
    Hotkey,
    /// Switches between the original page and the "Other" options.
    Other,
    Exit,
    Help,
    MaxFps(u16),
    ToggleInfo,
    ToggleFullscreen,
}

// GUI.ini [138-1530]/[138-1532]: the music and sound bars, and the two
// window sizes beside their icons. Clicks land on a taller band than the
// thin bars themselves.
const MUSIC_BAR: [i32; 4] = [84, 43, 88, 6];
const SOUND_BAR: [i32; 4] = [239, 43, 88, 6];
const BAR_HIT_PADDING: i32 = 6;
const RESOLUTION_RECTS: [(u8, [i32; 4]); 2] = [(1, [34, 62, 128, 22]), (2, [194, 62, 124, 22])];
/// The middle of the window, which the "Other" page takes over: the
/// dialog art paints Show Name and Shadow there.
const OTHER_PAGE_RECT: [i32; 4] = [28, 104, 314, 86];

/// Label and width of each frame-cap button.
fn fps_button(cap: u16) -> (&'static str, i32) {
    match cap {
        0 => ("VSync", 52),
        30 => ("30", 34),
        60 => ("60", 34),
        120 => ("120", 40),
        144 => ("144", 40),
        _ => ("Unlimited", 74),
    }
}

fn settings_controls(options: &crate::hud::HudConfig) -> Vec<Control<SettingsAction>> {
    use SettingsAction as A;
    let mut controls = vec![control(
        [340, 23, 15, 15],
        Face::Art("data/main/CloseBtnS.dds"),
        None,
    )];
    if options.options_page == 0 {
        controls.extend([
            control(
                [66, 124, 75, 20],
                Face::Check(
                    "data/main/SetupDisable.dds",
                    "data/main/SetupEnable.dds",
                    options.show_names,
                ),
                Some(A::ToggleNames),
            ),
            control(
                [208, 124, 75, 20],
                Face::Check(
                    "data/main/CountDisable.dds",
                    "data/main/CountEnable.dds",
                    options.kill_counter,
                ),
                Some(A::ToggleCounter),
            ),
            control(
                [153, 156, 60, 20],
                Face::Art("data/main/SetupBtnReal.dds"),
                Some(A::Shadows(true)),
            ),
            control(
                [244, 156, 60, 20],
                Face::Art("data/main/SetupBtnSimple.dds"),
                Some(A::Shadows(false)),
            ),
        ]);
    } else {
        let mut x = 40;
        for cap in crate::hud::FPS_CAPS {
            let (label, width) = fps_button(cap);
            controls.push(control(
                [x, 128, width, 20],
                Face::Button(label, options.max_fps == cap),
                Some(A::MaxFps(cap)),
            ));
            x += width + 4;
        }
        controls.extend([
            control(
                [42, 160, 140, 20],
                Face::Toggle("Show FPS / ping", options.show_info),
                Some(A::ToggleInfo),
            ),
            control(
                [200, 160, 130, 20],
                Face::Toggle("Fullscreen", options.fullscreen),
                Some(A::ToggleFullscreen),
            ),
        ]);
    }
    controls.extend([
        control(
            [45, 200, 62, 20],
            Face::Art("data/main/SetupBtnDelete.dds"),
            Some(A::Delete),
        ),
        control(
            [119, 200, 62, 20],
            Face::Art("data/main/SetupBtnHotkey.dds"),
            Some(A::Hotkey),
        ),
        control(
            [193, 200, 62, 20],
            Face::Art("data/main/SetupBtnOther.dds"),
            Some(A::Other),
        ),
        control(
            [267, 200, 62, 20],
            Face::Art("data/main/SetupBtnExit.dds"),
            Some(A::Exit),
        ),
        control([77, 261, 62, 20], Face::Art("data/main/HelpBtn2.dds"), Some(A::Help)),
        control(
            [244, 261, 62, 20],
            Face::Art("data/main/CloseBtn2.dds"),
            None,
        ),
    ]);
    controls
}

fn bar_value(bar: [i32; 4], point: [i32; 2]) -> Option<u8> {
    let band = [bar[0] - 4, bar[1] - BAR_HIT_PADDING, bar[2] + 8, bar[3] + BAR_HIT_PADDING * 2];
    inside(band, point).then(|| {
        (((point[0] - bar[0]) as f32 / bar[2] as f32).clamp(0.0, 1.0) * 100.0).round() as u8
    })
}

/// The Options window's action under the cursor.
pub fn settings_action_at(
    viewport: [f32; 2],
    cursor: [f32; 2],
    options: &crate::hud::HudConfig,
) -> Option<SettingsAction> {
    let point = local(QuickPanel::Settings, viewport, cursor)?;
    if let Some(volume) = bar_value(MUSIC_BAR, point) {
        return Some(SettingsAction::Music(volume));
    }
    if let Some(volume) = bar_value(SOUND_BAR, point) {
        return Some(SettingsAction::Sound(volume));
    }
    if let Some((index, _)) = RESOLUTION_RECTS.iter().find(|(_, rect)| inside(*rect, point)) {
        return Some(SettingsAction::Resolution(*index));
    }
    hit(&settings_controls(options), point)
}

/// The dynamic parts of the Options window: volume fills, the chosen window
/// size and shadow mode, and the second page's heading.
fn paint_settings(image: &mut RgbaImage, options: &crate::hud::HudConfig) {
    for (bar, volume) in [(MUSIC_BAR, options.music_volume), (SOUND_BAR, options.sound_volume)] {
        let filled = (bar[2] as f32 * f32::from(volume) / 100.0).round() as i32;
        // A gold gradient, darker at the bottom, and a round knob.
        for row in 0..bar[3] {
            let t = row as f32 / (bar[3] - 1).max(1) as f32;
            let colour = [
                (255.0 - 40.0 * t) as u8,
                (226.0 - 80.0 * t) as u8,
                (140.0 - 90.0 * t) as u8,
                235,
            ];
            fill(image, [bar[0], bar[1] + row, filled, 1], colour);
        }
        knob(image, [bar[0] + filled, bar[1] + bar[3] / 2]);
    }
    for (index, rect) in RESOLUTION_RECTS {
        if options.resolution == index {
            highlight(image, rect);
        }
    }
    if options.options_page == 0 {
        let selected = if options.full_shadows { [153, 156, 60, 20] } else { [244, 156, 60, 20] };
        highlight(image, selected);
    } else {
        highlight(image, [193, 200, 62, 20]);
    }
}

fn mail_controls() -> Vec<Control<()>> {
    vec![
        control(
            [372, 20, 16, 16],
            Face::Art("data/main/CloseBtnS.dds"),
            None,
        ),
        control(
            [222, 192, 62, 21],
            Face::Art("data/main/MsgSendAllMsg.dds"),
            None,
        ),
        control(
            [295, 192, 62, 21],
            Face::Art("data/main/MsgSendSend.dds"),
            None,
        ),
        control([64, 230, 46, 24], Face::Art("data/main/HelpBtn2.dds"), None),
        control(
            [290, 230, 64, 24],
            Face::Art("data/main/CloseBtn2.dds"),
            None,
        ),
    ]
}

// ---------------------------------------------------------------------------
// Framed panels built from the friend dialog art

/// `GeneralBG.dds` (the friend dialog) is 208 px wide; its frame band is
/// the top 40 px (title) and the bottom 20 px, and its list area spans
/// x 22..172 from y 40.
pub const FRAMED_WIDTH: u32 = 208;
pub const FRAMED_TOP: u32 = 40;
pub const FRAMED_BOTTOM: u32 = 20;
pub const FRAMED_LIST_X: [i32; 2] = [22, 172];
const GENERAL_BG: &str = "data/main/GeneralBG.dds";
const GENERAL_BG_LIST: [u32; 2] = [40, 290];
/// The dialog's drawn height; the DDS can be padded below it.
const GENERAL_BG_HEIGHT: u32 = 375;

/// A panel in the friend dialog's art at its native width, `inner` pixels
/// tall between the title band and the bottom frame: the list area is
/// stretched vertically and the frame bands are kept as drawn.
pub fn framed_panel(assets: &mut Assets, inner: u32) -> Result<RgbaImage> {
    let source = assets.image(GENERAL_BG)?;
    let width = source.width().min(FRAMED_WIDTH);
    let mut image = RgbaImage::new(FRAMED_WIDTH, FRAMED_TOP + inner + FRAMED_BOTTOM);
    let top = imageops::crop_imm(&source, 0, 0, width, FRAMED_TOP).to_image();
    imageops::overlay(&mut image, &top, 0, 0);
    let list = imageops::crop_imm(
        &source,
        0,
        GENERAL_BG_LIST[0],
        width,
        GENERAL_BG_LIST[1] - GENERAL_BG_LIST[0],
    )
    .to_image();
    let list = imageops::resize(&list, width, inner.max(1), imageops::FilterType::Triangle);
    imageops::overlay(&mut image, &list, 0, i64::from(FRAMED_TOP));
    let bottom = imageops::crop_imm(
        &source,
        0,
        source.height().min(GENERAL_BG_HEIGHT).saturating_sub(FRAMED_BOTTOM),
        width,
        FRAMED_BOTTOM,
    )
    .to_image();
    imageops::overlay(&mut image, &bottom, 0, i64::from(FRAMED_TOP + inner));
    Ok(image)
}

/// `GeneralBG.dds` cut for any size: the frame drawn on its 205x374
/// canvas, its borders (left, right, top with the title band, bottom).
const GENERAL_BG_FRAME: [u32; 2] = [205, 374];
const GENERAL_BG_BORDERS: [u32; 4] = [16, 17, 36, 38];

/// The friend dialog's frame at any size: corners as drawn, the patterned
/// bands repeated along each side, and the middle stretched to fit.
pub fn framed_window(assets: &mut Assets, width: u32, height: u32) -> Result<RgbaImage> {
    Ok(framed_window_from(&assets.image(GENERAL_BG)?, width, height))
}

/// The `GeneralBG.dds` art the framed windows are cut from.
pub fn general_bg(assets: &mut Assets) -> Result<RgbaImage> {
    assets.image(GENERAL_BG)
}

/// [`framed_window`] from an already loaded `GeneralBG.dds`.
pub fn framed_window_from(source: &RgbaImage, width: u32, height: u32) -> RgbaImage {
    let [frame_w, frame_h] = GENERAL_BG_FRAME;
    let frame = imageops::crop_imm(source, 0, 0, frame_w.min(source.width()), frame_h.min(source.height())).to_image();
    let (frame_w, frame_h) = frame.dimensions();
    let [left, right, top, bottom] = GENERAL_BG_BORDERS;
    let width = width.max(left + right + 1);
    let height = height.max(top + bottom + 1);
    let mut image = RgbaImage::new(width, height);
    let piece = |x: u32, y: u32, w: u32, h: u32| imageops::crop_imm(&frame, x, y, w, h).to_image();
    let (inner_w, inner_h) = (frame_w - left - right, frame_h - top - bottom);
    let (target_w, target_h) = (width - left - right, height - top - bottom);
    // Middle, stretched.
    let middle = imageops::resize(&piece(left, top, inner_w, inner_h), target_w, target_h, imageops::FilterType::Triangle);
    imageops::overlay(&mut image, &middle, i64::from(left), i64::from(top));
    // Bands, repeated so their pattern is not smeared.
    for (band, y) in [(piece(left, 0, inner_w, top), 0), (piece(left, frame_h - bottom, inner_w, bottom), height - bottom)] {
        let mut x = left;
        while x < width - right {
            let span = (width - right - x).min(inner_w);
            imageops::overlay(&mut image, &imageops::crop_imm(&band, 0, 0, span, band.height()).to_image(), i64::from(x), i64::from(y));
            x += span;
        }
    }
    for (band, x) in [(piece(0, top, left, inner_h), 0), (piece(frame_w - right, top, right, inner_h), width - right)] {
        let mut y = top;
        while y < height - bottom {
            let span = (height - bottom - y).min(inner_h);
            imageops::overlay(&mut image, &imageops::crop_imm(&band, 0, 0, band.width(), span).to_image(), i64::from(x), i64::from(y));
            y += span;
        }
    }
    // Corners, as drawn.
    for (x, y, w, h, dx, dy) in [
        (0, 0, left, top, 0, 0),
        (frame_w - right, 0, right, top, width - right, 0),
        (0, frame_h - bottom, left, bottom, 0, height - bottom),
        (frame_w - right, frame_h - bottom, right, bottom, width - right, height - bottom),
    ] {
        imageops::overlay(&mut image, &piece(x, y, w, h), i64::from(dx), i64::from(dy));
    }
    image
}

/// The panels' gold label text, centred on `center_x`.
pub fn label_centered(image: &mut RgbaImage, value: &str, size: f32, center_x: i32, y: i32, colour: [u8; 4]) {
    if let Ok(font) = crate::main_hud::label_font() {
        let width = crate::main_hud::text_width(font, value, size);
        crate::main_hud::crisp_text(image, font, value, size, [center_x - width / 2, y], colour);
    }
}

/// Like `label_centered`, but shrinks the text until it fits `max_width`.
pub fn label_fitted(image: &mut RgbaImage, value: &str, size: f32, center_x: i32, y: i32, colour: [u8; 4], max_width: i32) {
    if let Ok(font) = crate::main_hud::label_font() {
        let mut size = size;
        while size > 9.0 && crate::main_hud::text_width(font, value, size) > max_width {
            size -= 0.5;
        }
        let width = crate::main_hud::text_width(font, value, size);
        crate::main_hud::crisp_text(image, font, value, size, [center_x - width / 2, y], colour);
    }
}

/// The trade dialog's width; the partner card sits to its right.
pub const TRADE_WIDTH: f32 = 335.0;
pub const TRADE_PARTNER_INNER: u32 = 214;
/// Where the partner's model stands on its card: [top centre, [max width,
/// height]].
pub const TRADE_PARTNER_MODEL: [[f32; 2]; 2] = [[97.0, 52.0], [70.0, 150.0]];

/// A player card: the name in the title band, room for their live model
/// (drawn over it by the renderer) and their level at the bottom of the
/// `inner`-pixel list area.
pub fn player_card(assets: &mut Assets, name: &str, level: Option<u16>, inner: u32) -> Result<RgbaImage> {
    let mut image = framed_panel(assets, inner)?;
    let center = (FRAMED_LIST_X[0] + FRAMED_LIST_X[1]) / 2;
    label_centered(&mut image, name, 14.0, center, 12, LABEL_GOLD);
    let bottom = (FRAMED_TOP + inner) as i32;
    fill(&mut image, [FRAMED_LIST_X[0], bottom - 30, FRAMED_LIST_X[1] - FRAMED_LIST_X[0], 24], [0, 0, 0, 110]);
    let caption = level.map_or_else(|| "Player".to_owned(), |level| format!("Level {level}"));
    label_centered(&mut image, &caption, 13.0, center, bottom - 26, [255, 220, 120, 255]);
    Ok(image)
}

/// The colour of the original panels' text buttons.
pub const LABEL_GOLD: [u8; 4] = [232, 206, 148, 255];

// ---------------------------------------------------------------------------
// Painting

pub struct PanelData<'a> {
    pub character: &'a crate::character::Character,
    pub team_members: &'a [crate::network::TeamMember],
    pub team_leader_id: Option<u32>,
    pub team: TeamState,
    pub guild_members: &'a [crate::network::GuildMember],
    pub pending_guild_invite: Option<u32>,
    pub friends: &'a [crate::network::SocialEntry],
    pub ignores: &'a [crate::network::SocialEntry],
    /// The player whose friend request waits for Accept/Refuse.
    pub pending_friend_name: Option<&'a str>,
    pub selected_target_name: Option<&'a str>,
    pub trade: &'a crate::network::TradeView,
    /// The player's options, for the Options window.
    pub options: crate::hud::HudConfig,
    /// When the account's VIP ends (seconds since 1970; 0 without VIP).
    pub vip_until: u64,
}

const WHITE: [u8; 4] = [255, 255, 255, 255];
const GREY: [u8; 4] = [170, 170, 170, 255];
const GOLD: [u8; 4] = [255, 220, 120, 255];

fn text(image: &mut RgbaImage, value: &str, x: i32, y: i32, colour: [u8; 4], width: i32) {
    if let Ok(font) = crate::main_hud::font() {
        crate::main_hud::text(image, font, value, 12.0, [x, y], colour, Some(width));
    }
}

fn outline(image: &mut RgbaImage, rect: [i32; 4], colour: [u8; 4]) {
    let [x, y, width, height] = rect;
    let mut put = |px: i32, py: i32| {
        if px >= 0 && py >= 0 && (px as u32) < image.width() && (py as u32) < image.height() {
            image
                .get_pixel_mut(px as u32, py as u32)
                .blend(&Rgba(colour));
        }
    };
    for px in x..x + width {
        put(px, y);
        put(px, y + height - 1);
    }
    for py in y..y + height {
        put(x, py);
        put(x + width - 1, py);
    }
}

/// The lit state of a chosen option: a warm wash, a gold frame and a soft
/// halo one pixel outside it.
fn highlight(image: &mut RgbaImage, rect: [i32; 4]) {
    let [x, y, width, height] = rect;
    fill(image, rect, [255, 200, 90, 46]);
    outline(image, [x - 1, y - 1, width + 2, height + 2], [255, 200, 90, 90]);
    outline(image, rect, [255, 222, 130, 255]);
}

/// A round gold slider knob with a highlight, centred on `centre`.
fn knob(image: &mut RgbaImage, centre: [i32; 2]) {
    for dy in -5..=5_i32 {
        for dx in -5..=5_i32 {
            let distance = ((dx * dx + dy * dy) as f32).sqrt();
            if distance > 5.0 {
                continue;
            }
            let colour = if distance > 4.0 {
                [50, 34, 8, 255]
            } else if dx + dy < -2 {
                [255, 246, 210, 255]
            } else {
                [230, 180, 80, 255]
            };
            fill(image, [centre[0] + dx, centre[1] + dy, 1, 1], colour);
        }
    }
}

/// Blends `colour` over the rectangle.
pub fn fill(image: &mut RgbaImage, rect: [i32; 4], colour: [u8; 4]) {
    let [x, y, width, height] = rect;
    for py in y.max(0)..(y + height).min(image.height() as i32) {
        for px in x.max(0)..(x + width).min(image.width() as i32) {
            image
                .get_pixel_mut(px as u32, py as u32)
                .blend(&Rgba(colour));
        }
    }
}

fn item_icon(assets: &mut Assets, item_type: u32) -> Option<RgbaImage> {
    let icon = crate::shop::item_icon_native(assets, item_type).ok()?;
    let size = crate::original_ui_layout::INVENTORY_ICON_SIZE as u32;
    Some(imageops::resize(
        &icon,
        size,
        size,
        imageops::FilterType::Triangle,
    ))
}

pub fn image(assets: &mut Assets, panel: QuickPanel, data: &PanelData) -> Result<RgbaImage> {
    let frame = frame(panel).ok_or("panel has no original dialog")?;
    let mut image = RgbaImage::new(frame.size[0], frame.size[1]);
    crate::original_ui::overlay_asset(&mut image, assets, frame.art, 0, 0)?;
    match panel {
        QuickPanel::Vip => draw_vip(&mut image, assets, data.vip_until)?,
        QuickPanel::Friends => {
            paint(
                &mut image,
                assets,
                &friend_controls(data.pending_friend_name.is_some()),
            )?;
            for (row, entry) in friend_rows(data.friends.len(), data.ignores.len())
                .into_iter()
                .enumerate()
            {
                let [x, y, width, _] = friend_row_rect(row);
                let (name, colour) = match entry {
                    FriendRow::Friend(index) => {
                        let friend = &data.friends[index];
                        (
                            friend.name.as_str(),
                            if friend.online { WHITE } else { GREY },
                        )
                    }
                    FriendRow::Ignore(index) => {
                        (data.ignores[index].name.as_str(), [220, 110, 110, 255])
                    }
                };
                text(&mut image, name, x + 2, y + 2, colour, width - 14);
                text(
                    &mut image,
                    "x",
                    x + width - 10,
                    y + 1,
                    [210, 160, 120, 255],
                    12,
                );
            }
            if let Some(name) = data.pending_friend_name {
                text(&mut image, &format!("Request: {name}"), 24, 292, GOLD, 150);
            } else if let Some(target) = data.selected_target_name {
                text(&mut image, &format!("Target: {target}"), 24, 292, GOLD, 150);
            }
        }
        QuickPanel::Team => {
            paint(&mut image, assets, &team_controls(&data.team))?;
            for (member, x) in data.team_members.iter().zip(TEAM_PORTRAITS) {
                let rect = [x, 10, 47, 47];
                if let Some(face) = crate::renderer::player_face(assets, member.mesh, 43) {
                    imageops::overlay(&mut image, &face, (x + 2).into(), 12);
                }
                let name = if member.name.is_empty() {
                    member.member_id.to_string()
                } else {
                    member.name.clone()
                };
                fill(&mut image, [x + 2, 40, 43, 15], [0, 0, 0, 150]);
                text(&mut image, &name, x + 3, 41, WHITE, 42);
                let health = if member.maximum_health == 0 {
                    0.0
                } else {
                    (f32::from(member.health) / f32::from(member.maximum_health)).clamp(0.0, 1.0)
                };
                fill(&mut image, [x + 3, 53, 41, 2], [40, 10, 10, 255]);
                fill(
                    &mut image,
                    [x + 3, 53, (41.0 * health).round() as i32, 2],
                    [220, 40, 40, 255],
                );
                if data.team_leader_id == Some(member.member_id) {
                    outline(&mut image, rect, [255, 215, 108, 255]);
                }
            }
        }
        QuickPanel::Guild => {
            let has_guild = !data.character.guild.is_empty() || !data.guild_members.is_empty();
            paint(
                &mut image,
                assets,
                &guild_controls(has_guild, data.pending_guild_invite.is_some()),
            )?;
            let or_none = |value: &str| {
                if value.is_empty() {
                    "None".to_owned()
                } else {
                    value.to_owned()
                }
            };
            let leader = data
                .guild_members
                .iter()
                .find(|member| member.rank == 1000)
                .map(|member| member.name.clone())
                .unwrap_or_else(|| "None".to_owned());
            let online = data
                .guild_members
                .iter()
                .filter(|member| member.online)
                .count();
            // [133-1037..1042] value column at x=122.
            for (label, value, y) in [
                ("Guild:", or_none(&data.character.guild), 69),
                ("Position:", or_none(&data.character.guild_rank), 89),
                ("Leader:", leader, 109),
                (
                    "Members:",
                    format!("{online}/{}", data.guild_members.len()),
                    129,
                ),
            ] {
                text(&mut image, label, 32, y, GOLD, 88);
                text(&mut image, &value, 122, y, WHITE, 100);
            }
            if !has_guild {
                let line = match data.pending_guild_invite {
                    Some(inviter) => format!("Invitation from {inviter}"),
                    None => "You are not in a guild".to_owned(),
                };
                text(&mut image, &line, 32, GUILD_MEMBER_TOP, GREY, 190);
            }
            for (row, member) in data
                .guild_members
                .iter()
                .take(GUILD_MEMBER_ROWS)
                .enumerate()
            {
                let y = GUILD_MEMBER_TOP + row as i32 * GUILD_MEMBER_ROW;
                let colour = if member.online { WHITE } else { GREY };
                text(&mut image, &member.name, 32, y, colour, 110);
                text(
                    &mut image,
                    &format!("Lv.{}", member.level),
                    160,
                    y,
                    colour,
                    50,
                );
            }
        }
        QuickPanel::Trade => {
            let trade = data.trade;
            paint(
                &mut image,
                assets,
                &trade_controls(
                    trade.active,
                    trade.pending_request.is_some(),
                    trade.own_items.len(),
                ),
            )?;
            let partner = match (trade.peer_id, trade.pending_request) {
                _ if !trade.peer_name.is_empty() => trade.peer_name.clone(),
                (Some(peer), _) => peer.to_string(),
                (None, Some(requester)) => requester.to_string(),
                (None, None) => data.selected_target_name.unwrap_or("").to_owned(),
            };
            text(&mut image, &partner, 24, 29, WHITE, 120);
            if trade.peer_confirmed {
                text(&mut image, "Agreed", 158, 29, GOLD, 60);
            }
            for (grid, items) in [
                (TRADE_PEER_GRID, &trade.peer_items),
                (TRADE_OWN_GRID, &trade.own_items),
            ] {
                for (index, item) in items.iter().take(TRADE_SLOTS).enumerate() {
                    let [x, y, _, _] = trade_slot_rect(grid, index);
                    if let Some(icon) = item_icon(assets, item.item_type) {
                        imageops::overlay(&mut image, &icon, (x + 1).into(), (y + 1).into());
                    }
                }
            }
            // [137-1142] partner silver, [137-1570] own silver.
            text(
                &mut image,
                &trade.peer_silver.to_string(),
                219,
                76,
                WHITE,
                84,
            );
            // [137-1150] partner CPs, [137-1573] own CPs.
            text(&mut image, &trade.peer_cps.to_string(), 219, 101, WHITE, 84);
            for (amount, rect) in [
                (TradeAmount::Silver, TRADE_OWN_SILVER),
                (TradeAmount::Cps, TRADE_OWN_CPS),
            ] {
                text(
                    &mut image,
                    &trade.amount_text(amount),
                    rect[0] + 4,
                    rect[1] + 1,
                    WHITE,
                    84,
                );
                if trade.editing == Some(amount) {
                    outline(&mut image, rect, [255, 215, 108, 255]);
                }
            }
            if trade.own_confirmed {
                outline(&mut image, [112, 261, 62, 20], [255, 215, 108, 255]);
            }
        }
        QuickPanel::Settings => {
            if data.options.options_page != 0 {
                // The original middle rows give way to the client's own
                // options, on a framed plate over the art.
                fill(&mut image, OTHER_PAGE_RECT, [10, 26, 40, 255]);
                outline(&mut image, OTHER_PAGE_RECT, [150, 124, 70, 255]);
                text(&mut image, "FPS limit", 42, 110, GOLD, 120);
            }
            paint(&mut image, assets, &settings_controls(&data.options))?;
            paint_settings(&mut image, &data.options);
        }
        QuickPanel::Mail => {
            paint(&mut image, assets, &mail_controls())?;
            text(&mut image, "No messages.", 46, 80, GREY, 290);
        }
        _ => {}
    }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controls_are_hit_where_they_are_painted() {
        let viewport = [1366.0, 768.0];
        let at = |panel: QuickPanel, rect: [i32; 4]| {
            let [left, top] = position(panel, viewport).expect("original panel");
            [
                left + rect[0] as f32 + rect[2] as f32 * 0.5,
                top + rect[1] as f32 + rect[3] as f32 * 0.5,
            ]
        };
        assert_eq!(
            friends_action_at(
                viewport,
                at(QuickPanel::Friends, [70, 308, 63, 20]),
                0,
                0,
                true
            ),
            Some(FriendsPanelAction::AddFriend)
        );
        // Accept answers a received request and is not there without one.
        let accept = at(QuickPanel::Friends, [140, 308, 52, 20]);
        assert_eq!(
            friends_action_at(viewport, accept, 0, 0, true),
            Some(FriendsPanelAction::AcceptRequest)
        );
        assert_eq!(friends_action_at(viewport, accept, 0, 0, false), None);
        assert_eq!(
            friends_action_at(
                viewport,
                at(QuickPanel::Friends, friend_row_rect(1)),
                1,
                1,
                false
            ),
            Some(FriendsPanelAction::RemoveIgnore(0))
        );
        let team = TeamState {
            has_team: false,
            pending_invite: true,
            pending_join_request: false,
            join_enabled: true,
        };
        assert_eq!(
            team_action_at(viewport, at(QuickPanel::Team, [68, 60, 43, 17]), &team),
            Some(TeamPanelAction::AcceptInvite)
        );
        assert_eq!(
            team_portrait_at(viewport, at(QuickPanel::Team, [113, 10, 47, 47])),
            Some(2)
        );
        assert_eq!(
            guild_action_at(
                viewport,
                at(QuickPanel::Guild, [46, 320, 64, 22]),
                true,
                false
            ),
            Some(GuildPanelAction::Leave)
        );
        assert!(close_at(
            QuickPanel::Guild,
            viewport,
            at(QuickPanel::Guild, [144, 350, 63, 21])
        ));
        let mut trade = crate::network::TradeView::default();
        trade.active = true;
        assert_eq!(
            trade_action_at(
                viewport,
                at(QuickPanel::Trade, trade_slot_rect(TRADE_OWN_GRID, 0)),
                &trade
            ),
            Some(TradePanelAction::AddItem)
        );
        assert_eq!(
            trade_action_at(viewport, at(QuickPanel::Trade, TRADE_OWN_SILVER), &trade),
            Some(TradePanelAction::EditAmount(TradeAmount::Silver))
        );
        assert_eq!(
            trade_action_at(viewport, at(QuickPanel::Trade, TRADE_OWN_CPS), &trade),
            Some(TradePanelAction::EditAmount(TradeAmount::Cps))
        );
        assert!(!is_original(QuickPanel::Market));
    }

    #[test]
    #[ignore = "Requires a local 5065 installation; exports the original panels"]
    fn export_preview() -> Result<()> {
        let root = std::env::var("GRAVAK_ASSETS")?;
        let output = std::path::PathBuf::from(std::env::var("GRAVAK_PANEL_PREVIEW")?);
        std::fs::create_dir_all(&output)?;
        let mut assets = Assets::open(std::path::Path::new(&root))?;
        let mut character = crate::character::Character::load(&mut assets, 2)?;
        character.guild = "Elite".into();
        character.guild_rank = "Guild Leader".into();
        let friends = [("Sheko", true), ("CloudStrider", false)]
            .map(|(name, online)| crate::network::SocialEntry {
                friend_id: 1,
                name: name.into(),
                online,
            })
            .to_vec();
        let ignores = vec![crate::network::SocialEntry {
            friend_id: 9,
            name: "Spammer".into(),
            online: true,
        }];
        let team_members = vec![crate::network::TeamMember {
            member_id: 7,
            name: "Ikzordep".into(),
            mesh: 0,
            health: 60,
            maximum_health: 100,
        }];
        let guild_members = vec![crate::network::GuildMember {
            character_id: 7,
            name: "Ikzordep".into(),
            mesh: 0,
            level: 24,
            rank: 1000,
            donation: 0,
            online: true,
        }];
        let mut trade = crate::network::TradeView::default();
        trade.active = true;
        trade.own_silver = 1_500;
        for panel in [
            QuickPanel::Friends,
            QuickPanel::Team,
            QuickPanel::Guild,
            QuickPanel::Trade,
            QuickPanel::Settings,
            QuickPanel::Mail,
        ] {
            let data = PanelData {
                character: &character,
                team_members: &team_members,
                team_leader_id: Some(7),
                team: TeamState {
                    has_team: true,
                    pending_invite: false,
                    pending_join_request: true,
                    join_enabled: true,
                },
                guild_members: &guild_members,
                pending_guild_invite: None,
                friends: &friends,
                ignores: &ignores,
                pending_friend_name: None,
                selected_target_name: Some("Sheko"),
                trade: &trade,
                options: crate::hud::HudConfig::default(),
                vip_until: 0,
            };
            image(&mut assets, panel, &data)?.save(output.join(format!("{panel:?}.png")))?;
        }
        Ok(())
    }

}

#[cfg(test)]
mod vip_preview {
    /// `BOUNTY_PREVIEW=dir cargo test -p conquer-sandbox vip_window_preview`
    #[test]
    fn vip_window_preview() {
        let Ok(dir) = std::env::var("BOUNTY_PREVIEW") else { return };
        let mut assets = crate::assets::Assets::open(std::path::Path::new("R:/5065")).expect("assets");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs());
        for (name, until) in [("active", now + 6 * 86_400 + 3 * 3_600), ("inactive", 0)] {
            let mut image = image::RgbaImage::new(330, 200);
            crate::original_ui::overlay_asset(&mut image, &mut assets, "data/main/VipInfoBG.dds", 0, 0).expect("art");
            super::draw_vip(&mut image, &mut assets, until).expect("vip");
            image.save(format!("{dir}/vip-window-{name}.png")).expect("png");
        }
    }
}
