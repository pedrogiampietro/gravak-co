//! Social and utility windows (Friend, Team, Guild, Trade, Options and
//! Message) rebuilt from their original 5065 dialogs.
//!
//! Each window is described by a frame (`Control.Ani` dialog art, size and
//! `GUI.ini` anchor) and by a list of controls computed from the live state.
//! The same list is used to paint the window and to hit-test clicks, so the
//! two cannot drift apart.

use crate::assets::{Assets, Result};
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
    /// Original dialog art, or `None` for windows painted by the client.
    art: Option<&'static str>,
    size: [u32; 2],
    anchor: Anchor,
    /// Small top-right close button and, optionally, a bottom "Close".
    close: &'static [[i32; 4]],
}

fn frame(panel: QuickPanel) -> Option<Frame> {
    Some(match panel {
        // [0-160] plus [160-1681] close.
        QuickPanel::Friends => Frame {
            art: Some("data/main/GeneralBG.dds"),
            size: [208, 375],
            anchor: Anchor::Left,
            close: &[[175, 10, 15, 15]],
        },
        // [0-141] plus [141-1642] close.
        QuickPanel::Team => Frame {
            art: Some("data/main/DialogGroup.dds"),
            size: [290, 114],
            anchor: Anchor::TopCenter,
            close: &[[263, 10, 15, 15]],
        },
        // [0-133] plus [133-1510]/[133-1511].
        QuickPanel::Guild => Frame {
            art: Some("data/main/DialogQuery_Faction.dds"),
            size: [250, 392],
            anchor: Anchor::Left,
            close: &[[210, 23, 15, 15], [144, 350, 63, 21]],
        },
        // [0-137] plus [137-1138].
        QuickPanel::Trade => Frame {
            art: Some("data/main/DialogTrade.dds"),
            size: [335, 297],
            anchor: Anchor::Left,
            close: &[[303, 8, 15, 15]],
        },
        // Incoming trade request: a small accept/refuse prompt shown before
        // the full trade window.
        QuickPanel::TradeRequest => Frame {
            art: None,
            size: TRADE_REQUEST_SIZE,
            anchor: Anchor::Center,
            close: &[TRADE_REQUEST_CLOSE],
        },
        // [0-138] plus [138-1535]/[138-1531].
        QuickPanel::Settings => Frame {
            art: Some("data/main/DialogSetup.dds"),
            size: [370, 294],
            anchor: Anchor::Center,
            close: &[[340, 23, 15, 15], [244, 261, 62, 20]],
        },
        // [0-232] plus [232-1281]/[232-1283].
        QuickPanel::Mail => Frame {
            art: Some("data/main/DialogMsgSend.dds"),
            size: [396, 261],
            anchor: Anchor::Center,
            close: &[[372, 20, 16, 16], [290, 230, 64, 24]],
        },
        _ => return None,
    })
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
const TRADE_SLOTS: usize = 6;

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
            // [137-1570] own silver and [137-1145] "confirm amount".
            control([211, 187, 88, 16], Face::Label(""), Some(A::SetMoney)),
            control(
                [186, 236, 64, 22],
                Face::Art("data/main/AgreeBtn.dds"),
                Some(A::SetMoney),
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

/// Dropping an inventory item on the own grid ([137-1141]) offers it.
pub fn trade_drop_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    local(QuickPanel::Trade, viewport, cursor)
        .is_some_and(|point| inside([20, 167, 128, 86], point))
}

// ---------------------------------------------------------------------------
// Trade request prompt

const TRADE_REQUEST_SIZE: [u32; 2] = [260, 112];
const TRADE_REQUEST_CLOSE: [i32; 4] = [235, 7, 15, 15];
const TRADE_REQUEST_ACCEPT: [i32; 4] = [40, 76, 76, 22];
const TRADE_REQUEST_REFUSE: [i32; 4] = [144, 76, 76, 22];

fn trade_request_controls() -> Vec<Control<TradePanelAction>> {
    vec![
        control(
            TRADE_REQUEST_CLOSE,
            Face::Art("data/main/CloseBtnS.dds"),
            None,
        ),
        control(
            TRADE_REQUEST_ACCEPT,
            Face::Label("Accept"),
            Some(TradePanelAction::AcceptRequest),
        ),
        control(
            TRADE_REQUEST_REFUSE,
            Face::Label("Refuse"),
            Some(TradePanelAction::Cancel),
        ),
    ]
}

pub fn trade_request_action_at(viewport: [f32; 2], cursor: [f32; 2]) -> Option<TradePanelAction> {
    hit(
        &trade_request_controls(),
        local(QuickPanel::TradeRequest, viewport, cursor)?,
    )
}

/// Dark teal panel with the gold double border of the original dialogs.
fn trade_request_background(image: &mut RgbaImage) {
    let [width, height] = [image.width() as i32, image.height() as i32];
    fill(image, [0, 0, width, height], [18, 44, 54, 240]);
    fill(image, [3, 3, width - 6, 22], [28, 66, 80, 255]);
    outline(image, [0, 0, width, height], [10, 20, 26, 255]);
    outline(image, [1, 1, width - 2, height - 2], [196, 166, 96, 255]);
    outline(image, [3, 3, width - 6, height - 6], [92, 132, 140, 255]);
    fill(image, [4, 25, width - 8, 1], [92, 132, 140, 255]);
    for rect in [TRADE_REQUEST_ACCEPT, TRADE_REQUEST_REFUSE] {
        let [x, y, w, h] = rect;
        fill(image, rect, [12, 30, 38, 255]);
        outline(image, rect, [150, 126, 74, 255]);
        outline(image, [x + 1, y + 1, w - 2, h - 2], [40, 78, 90, 255]);
    }
}

// ---------------------------------------------------------------------------
// Options ([0-138]) and Message ([0-232]) have no client-side behaviour yet;
// they show the original controls only.

fn settings_controls() -> Vec<Control<()>> {
    let check = |rect, off, on| control(rect, Face::Check(off, on, true), None);
    vec![
        control(
            [340, 23, 15, 15],
            Face::Art("data/main/CloseBtnS.dds"),
            None,
        ),
        check(
            [66, 124, 75, 20],
            "data/main/SetupDisable.dds",
            "data/main/SetupEnable.dds",
        ),
        check(
            [208, 124, 75, 20],
            "data/main/CountDisable.dds",
            "data/main/CountEnable.dds",
        ),
        control(
            [153, 156, 60, 20],
            Face::Art("data/main/SetupBtnReal.dds"),
            None,
        ),
        control(
            [244, 156, 60, 20],
            Face::Art("data/main/SetupBtnSimple.dds"),
            None,
        ),
        control(
            [45, 200, 62, 20],
            Face::Art("data/main/SetupBtnDelete.dds"),
            None,
        ),
        control(
            [119, 200, 62, 20],
            Face::Art("data/main/SetupBtnHotkey.dds"),
            None,
        ),
        control(
            [193, 200, 62, 20],
            Face::Art("data/main/SetupBtnOther.dds"),
            None,
        ),
        control(
            [267, 200, 62, 20],
            Face::Art("data/main/SetupBtnExit.dds"),
            None,
        ),
        control([77, 261, 62, 20], Face::Art("data/main/HelpBtn2.dds"), None),
        control(
            [244, 261, 62, 20],
            Face::Art("data/main/CloseBtn2.dds"),
            None,
        ),
    ]
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

/// The panels' gold label text, centred on `center_x`.
pub fn label_centered(image: &mut RgbaImage, value: &str, size: f32, center_x: i32, y: i32, colour: [u8; 4]) {
    if let Ok(font) = crate::main_hud::label_font() {
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
    match frame.art {
        Some(art) => crate::original_ui::overlay_asset(&mut image, assets, art, 0, 0)?,
        None => trade_request_background(&mut image),
    }
    match panel {
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
            text(
                &mut image,
                &trade.own_silver.to_string(),
                215,
                188,
                WHITE,
                84,
            );
            if trade.own_confirmed {
                outline(&mut image, [112, 261, 62, 20], [255, 215, 108, 255]);
            }
        }
        QuickPanel::TradeRequest => {
            paint(&mut image, assets, &trade_request_controls())?;
            if let Ok(font) = crate::main_hud::label_font() {
                let title = "Trade";
                let width = crate::main_hud::text_width(font, title, 15.0);
                crate::main_hud::crisp_text(
                    &mut image,
                    font,
                    title,
                    15.0,
                    [(TRADE_REQUEST_SIZE[0] as i32 - width) / 2, 6],
                    [232, 206, 148, 255],
                );
            }
            let trade = data.trade;
            let requester = if trade.peer_name.is_empty() {
                trade
                    .pending_request
                    .map(|id| id.to_string())
                    .unwrap_or_default()
            } else {
                trade.peer_name.clone()
            };
            let centred = |image: &mut RgbaImage, value: &str, y: i32, colour: [u8; 4]| {
                if let Ok(font) = crate::main_hud::font() {
                    let max = TRADE_REQUEST_SIZE[0] as i32 - 24;
                    let width = crate::main_hud::text_width(font, value, 12.0).min(max);
                    text(
                        image,
                        value,
                        (TRADE_REQUEST_SIZE[0] as i32 - width) / 2,
                        y,
                        colour,
                        max,
                    );
                }
            };
            centred(&mut image, &requester, 34, GOLD);
            centred(&mut image, "is sending you a trade request.", 52, WHITE);
        }
        QuickPanel::Settings => paint(&mut image, assets, &settings_controls())?,
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
    fn trade_request_prompt_accepts_and_refuses() {
        let viewport = [1366.0, 768.0];
        let at = |rect: [i32; 4]| {
            let [left, top] = position(QuickPanel::TradeRequest, viewport).expect("prompt");
            [
                left + rect[0] as f32 + rect[2] as f32 * 0.5,
                top + rect[1] as f32 + rect[3] as f32 * 0.5,
            ]
        };
        assert_eq!(
            trade_request_action_at(viewport, at(TRADE_REQUEST_ACCEPT)),
            Some(TradePanelAction::AcceptRequest)
        );
        assert_eq!(
            trade_request_action_at(viewport, at(TRADE_REQUEST_REFUSE)),
            Some(TradePanelAction::Cancel)
        );
        assert!(close_at(
            QuickPanel::TradeRequest,
            viewport,
            at(TRADE_REQUEST_CLOSE)
        ));
    }

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
        assert!(trade_drop_at(
            viewport,
            at(QuickPanel::Trade, [20, 167, 128, 86])
        ));
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
            };
            image(&mut assets, panel, &data)?.save(output.join(format!("{panel:?}.png")))?;
        }
        Ok(())
    }

}
