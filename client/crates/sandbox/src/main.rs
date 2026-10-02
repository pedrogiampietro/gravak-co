// Source: @digitalm1nd on x.com / _dek on Discord — Preservation Conquer project — https://discord.gg/CvKPXEHYRY
mod action;
mod action_bar;
mod cursors;
mod assets;
mod audio;
mod character;
mod components;
mod debug_log;
mod hud;
mod labels;
mod main_hud;
mod magic;
mod map;
mod minimap;
mod motion_math;
mod movement;
mod network;
mod npc_dialog;
mod original_panels;
mod original_ui;
mod original_ui_layout;
mod renderer;
mod settings;
mod shop;
mod ui;
mod ui_theme;
mod validation;
use action::Action;
use assets::{Assets, Result};
use motion_math::{direction_towards, walk_delta};
use network::{ChatMessage, NetworkConfig, NetworkEvent, NetworkHandle, RemoteActor, SocialEntry};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
    sync::{Arc, mpsc},
    time::Instant,
};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorIcon, CustomCursor, Window, WindowId},
};
const LEVEL_UP_DURATION: f32 = 61.0 / 30.0;
// In normal play the presentation surface is synchronized to the monitor. A
// manual cap is still available through GRAVAK_MAX_FPS for benchmarking or
// machines where a fixed budget is preferred.
const DEFAULT_MAX_FPS: u32 = 0;

fn frame_interval_for(max_fps: u32) -> Option<std::time::Duration> {
    (max_fps > 0).then(|| std::time::Duration::from_secs_f64(1.0 / f64::from(max_fps)))
}

fn configured_frame_interval() -> Option<std::time::Duration> {
    let max_fps = std::env::var("GRAVAK_MAX_FPS")
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        .unwrap_or(DEFAULT_MAX_FPS);
    frame_interval_for(max_fps)
}

/// What the next click on a player does after a Friend/Trade button was
/// pressed with no player selected, like the original targeting cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TargetPick {
    Friend,
    Ignore,
    Trade,
}

impl TargetPick {
    /// The notice shown at the top of the screen while the pick waits.
    fn notice(self) -> &'static str {
        match self {
            Self::Friend => "Click a player to send a friend request (click the ground to cancel)",
            Self::Ignore => "Click a player to add as enemy (click the ground to cancel)",
            Self::Trade => "Click a player to request a trade (click the ground to cancel)",
        }
    }
}

/// Rows of the Shift+click player menu, in order.
const PLAYER_MENU: [(&str, PlayerMenuAction); 6] = [
    ("TRADE", PlayerMenuAction::Trade),
    ("FOLLOW", PlayerMenuAction::Follow),
    ("WHISPER", PlayerMenuAction::Whisper),
    ("ADD FRIEND", PlayerMenuAction::AddFriend),
    ("TEAM INVITE", PlayerMenuAction::TeamInvite),
    ("ENEMY", PlayerMenuAction::Enemy),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlayerMenuAction {
    Trade,
    Follow,
    Whisper,
    AddFriend,
    TeamInvite,
    Enemy,
}

/// The menu opened by Shift+clicking another player.
struct PlayerMenu {
    target_id: u32,
    name: String,
    menu: shop::ChoiceMenu,
}

enum RemoteModelJob {
    Player {
        id: u32,
        body: u32,
        armor: u32,
        helmet: u32,
        hair: u16,
        right_weapon: u32,
        left_weapon: u32,
        name: String,
        health: u16,
    },
    Npc {
        id: u32,
        kind: u16,
        mesh: u16,
    },
    Monster {
        id: u32,
        mesh: u32,
    },
}

enum RemoteModelResult {
    Player {
        id: u32,
        model: std::result::Result<character::Character, String>,
    },
    Npc {
        id: u32,
        model: std::result::Result<character::NpcModel, String>,
    },
    Monster {
        id: u32,
        model: std::result::Result<character::NpcModel, String>,
    },
}

struct RemoteModelLoader {
    requests: mpsc::Sender<RemoteModelJob>,
    results: mpsc::Receiver<RemoteModelResult>,
}

impl RemoteModelLoader {
    fn new(root: PathBuf, catalog: character::NpcCatalog) -> Result<Self> {
        let (requests, request_rx) = mpsc::channel::<RemoteModelJob>();
        let (result_tx, results) = mpsc::channel::<RemoteModelResult>();
        std::thread::Builder::new()
            .name("gravak-remote-model-loader".to_owned())
            .spawn(move || {
                let mut assets = match Assets::open(&root) {
                    Ok(assets) => assets,
                    Err(error) => {
                        let message = error.to_string();
                        while let Ok(job) = request_rx.recv() {
                            let result = match job {
                                RemoteModelJob::Player { id, .. } => RemoteModelResult::Player {
                                    id,
                                    model: Err(message.clone()),
                                },
                                RemoteModelJob::Npc { id, .. } => RemoteModelResult::Npc {
                                    id,
                                    model: Err(message.clone()),
                                },
                                RemoteModelJob::Monster { id, .. } => RemoteModelResult::Monster {
                                    id,
                                    model: Err(message.clone()),
                                },
                            };
                            if result_tx.send(result).is_err() {
                                return;
                            }
                        }
                        return;
                    }
                };
                while let Ok(job) = request_rx.recv() {
                    let result = match job {
                        RemoteModelJob::Player {
                            id,
                            body,
                            armor,
                            helmet,
                            hair,
                            right_weapon,
                            left_weapon,
                            name,
                            health,
                        } => RemoteModelResult::Player {
                            id,
                            model: character::Character::load_remote(
                                &mut assets,
                                body,
                                armor,
                                helmet,
                                hair,
                                right_weapon,
                                left_weapon,
                                name,
                                health,
                            )
                            .map_err(|error| error.to_string()),
                        },
                        RemoteModelJob::Npc { id, kind, mesh } => RemoteModelResult::Npc {
                            id,
                            model: character::NpcModel::load(&mut assets, &catalog, kind, mesh)
                                .map_err(|error| error.to_string()),
                        },
                        RemoteModelJob::Monster { id, mesh } => RemoteModelResult::Monster {
                            id,
                            model: character::NpcModel::load_monster(&mut assets, &catalog, mesh)
                                .map_err(|error| error.to_string()),
                        },
                    };
                    if result_tx.send(result).is_err() {
                        return;
                    }
                }
            })
            .map_err(|error| format!("failed to start remote model loader: {error}"))?;
        Ok(Self { requests, results })
    }
}

struct App {
    assets: Assets,
    map: map::Map,
    character: character::Character,
    action_bar: action_bar::ActionBar,
    action_context_slot: Option<usize>,
    item_context: Option<shop::ItemContext>,
    /// A Friend/Trade button waiting for the player to click its target.
    target_pick: Option<TargetPick>,
    /// A received friend request (requester id, name) awaiting Accept/Refuse.
    pending_friend_request: Option<(u32, String)>,
    /// The game's own cursors, created with the window.
    cursors: HashMap<cursors::GameCursor, CustomCursor>,
    /// The cursor last set on the window.
    shown_cursor: Option<cursors::GameCursor>,
    /// The Shift+click menu of another player.
    player_menu: Option<PlayerMenu>,
    /// The player the hero walks after (player menu Follow).
    follow_target: Option<u32>,
    /// The kill-mode menu above the Capture button is open.
    pk_menu_open: bool,
    /// The server's kill mode (MsgAction 96): 0 PK, 1 peace, 2 team, 3 capture.
    kill_mode: u8,
    item_action_panel: Option<shop::ItemActionPanel>,
    action_animation_started: Option<Instant>,
    gem_selection_panel: Option<shop::GemSelectionPanel>,
    last_inventory_click: Option<(u32, Instant)>,
    last_equipment_click: Option<(u32, Instant)>,
    quick_panel: Option<shop::QuickPanel>,
    player: movement::Player,
    audio: Option<audio::MovementAudio>,
    window: Option<Arc<Window>>,
    renderer: Option<renderer::Renderer>,
    cursor: [f32; 2],
    started: Instant,
    last: Instant,
    error: Option<String>,
    modifiers: winit::keyboard::ModifiersState,
    held: bool,
    queued_jump: Option<[i32; 2]>,
    local_action: Option<(Action, f32, f32)>,
    stamina: u8,
    /// When the player died; `None` while alive.
    dead_since: Option<f32>,
    /// Sitting (MsgAction 250): stamina comes back faster.
    sitting: bool,
    /// A portal works once the hero has stood off every portal, so arriving
    /// on the destination's own portal does not bounce straight back.
    portal_armed: bool,
    hit_until: f32,
    level_up_until: f32,
    level: u8,
    initial_sync_complete: bool,
    experience: u64,
    nobility_donation: u64,
    nobility_rank: u32,
    nobility_remaining: u64,
    equipped_title: u32,
    nobility_ranking: Vec<network::NobilityEntry>,
    status_flags: u64,
    status_open: bool,
    status_tab: shop::StatusTab,
    weapon_filter: shop::WeaponFilter,
    performance_overlay: bool,
    hud_config: hud::HudConfig,
    network: Option<NetworkHandle>,
    network_character_id: Option<u32>,
    hidden_default_equipment_slots: [bool; 10],
    equipment: Vec<network::OwnedItem>,
    inventory: Vec<network::OwnedItem>,
    inventory_open: bool,
    inventory_filter: shop::InventoryFilter,
    warehouse: Option<Vec<network::OwnedItem>>,
    warehouse_npc_id: Option<u32>,
    guild_warehouse_open: bool,
    warehouse_page: usize,
    selected_inventory_uid: Option<u32>,
    selected_warehouse_uid: Option<u32>,
    pending_inventory_move: Option<(network::OwnedItem, Option<network::OwnedItem>)>,
    /// Inventory sort is serialized: the server must confirm each swap before
    /// the next one is sent. Sending the whole plan at once makes the client
    /// race its own authoritative item updates.
    pending_inventory_sort: Option<VecDeque<(u32, u8)>>,
    pending_inventory_sort_move: Option<(u32, u8)>,
    pending_inventory_drop: Option<network::OwnedItem>,
    silver: u32,
    mana: u32,
    maximum_mana: u32,
    warehouse_silver: u32,
    cps: u32,
    warehouse_amount: String,
    warehouse_amount_focused: bool,
    chat_text: String,
    chat_focused: bool,
    chat_history: VecDeque<ChatMessage>,
    chat_filter: u16,
    friends: Vec<SocialEntry>,
    ignores: Vec<SocialEntry>,
    dragging_shop_item: Option<u32>,
    dragging_inventory_item: Option<network::OwnedItem>,
    inventory_drag_start_cursor: Option<[f32; 2]>,
    dragging_equipment_item: Option<network::OwnedItem>,
    confirmed_network_cell: Option<[i32; 2]>,
    pending_network_steps: VecDeque<[i32; 2]>,
    pending_network_jump: bool,
    pending_attack: Option<u32>,
    /// Monster being attacked with the selected skill (right click): the
    /// skill is recast whenever its cooldown allows, approaching first when
    /// the target is out of range.
    pending_skill_attack: Option<(u32, u32)>,
    selected_target: Option<u32>,
    pending_pickup: Option<(u32, u32, [i32; 2])>,
    skill_cooldown_until: HashMap<u32, f32>,
    /// The HUD settings last written to disk.
    saved_hud: Option<settings::HudSettings>,
    next_attack_at: f32,
    remote_actors: HashMap<u32, RemoteActor>,
    /// Life and maximum life of the Guild War structures.
    structure_lives: HashMap<u32, (u32, u32)>,
    remote_models: HashMap<u32, character::Character>,
    remote_npcs: HashMap<u32, character::NpcModel>,
    remote_monsters: HashMap<u32, character::NpcModel>,
    combat_numbers: Vec<(u32, u32, f32)>,
    skill_effects: Vec<renderer::SkillEffectInstance>,
    known_skills: HashMap<u32, (u16, u32)>,
    /// Weapon proficiency by weapon type: (level, experience).
    weapon_skills: std::collections::BTreeMap<u32, (u32, u32)>,
    /// Monsters killed by this character in the session (the KO counter).
    kill_count: u32,
    skill_bar_open: bool,
    /// Skill chosen in the skill bar; shown on the "Skill" button and cast
    /// with the right mouse button.
    active_skill: Option<u32>,
    team_members: Vec<network::TeamMember>,
    team_leader_id: Option<u32>,
    pending_team_invite: Option<u32>,
    pending_team_join_request: Option<u32>,
    team_join_enabled: bool,
    guild_members: Vec<network::GuildMember>,
    guild_id: Option<u32>,
    pending_guild_invite: Option<u32>,
    trade: network::TradeView,
    pending_trade_items: Vec<u32>,
    player_shop_items: Vec<network::PlayerShopItem>,
    player_shop_open: bool,
    player_shop_price: String,
    player_shop_price_focused: bool,
    ground_loot: Vec<(u32, [f32; 2], u32, f32)>,
    dialog: Option<DialogState>,
    shop: Option<shop::ShopState>,
    network_status: String,
    frame_interval: Option<std::time::Duration>,
    next_frame_at: Instant,
    remote_model_loader: Option<RemoteModelLoader>,
    remote_models_loading: HashSet<u32>,
    remote_npcs_loading: HashSet<u32>,
    remote_monsters_loading: HashSet<u32>,
}

/// The client offers Revive this long after dying (server `REVIVE_DELAY`).
const REVIVE_DELAY_SECONDS: f32 = 18.0;
/// Task id of the local Revive dialog; it never reaches an NPC.
const REVIVE_TASK_ID: u32 = u32::MAX;

struct DialogState {
    task_id: u32,
    /// NPC face (`ani/NpcFace.Ani`) sent with the avatar action.
    avatar: u16,
    lines: Vec<String>,
    options: Vec<(u8, String)>,
    /// A text box (MsgTaskDialog Edit): its option, label and what was
    /// typed, and whether the keyboard goes to it.
    edit: Option<(u8, String)>,
    input: String,
    input_focused: bool,
}

impl DialogState {
    /// Shows what is typed on the text box's row.
    fn refresh_edit_row(&mut self) {
        let Some((option_id, label)) = self.edit.clone() else {
            return;
        };
        let caret = if self.input_focused { "_" } else { "" };
        if let Some(row) = self.options.iter_mut().find(|(id, _)| *id == option_id) {
            row.1 = format!("{label}: {}{caret}", self.input);
        }
    }
}
impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let result = (|| -> Result<_> {
            let window = Arc::new(
                event_loop.create_window(
                    Window::default_attributes()
                        .with_title("gravak-co — offline Twin City sandbox")
                        .with_inner_size(winit::dpi::PhysicalSize::new(1366, 768)),
                )?,
            );
            let mut renderer = pollster::block_on(renderer::Renderer::new(window.clone()))?;
            renderer.prime_static_map_cache(&self.map);
            Ok((window, renderer))
        })();
        match result {
            Ok((window, renderer)) => {
                for cursor in cursors::GameCursor::ALL {
                    let [x, y] = cursor.hotspot();
                    let size = cursors::SIZE as u16;
                    match CustomCursor::from_rgba(cursor.image().into_raw(), size, size, x, y) {
                        Ok(source) => {
                            self.cursors.insert(cursor, event_loop.create_custom_cursor(source));
                        }
                        Err(error) => eprintln!("Cursor {cursor:?} unavailable: {error}"),
                    }
                }
                self.window = Some(window);
                self.renderer = Some(renderer);
                self.last = Instant::now();
                self.next_frame_at = self.last;
                if let Some(interval) = self.frame_interval {
                    eprintln!("FRAME_LIMIT max_fps={:.0}", 1.0 / interval.as_secs_f64());
                } else {
                    eprintln!("FRAME_LIMIT max_fps=unlimited");
                }
            }
            Err(e) => {
                self.error = Some(e.to_string());
                event_loop.exit();
            }
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size.width, size.height)
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = [position.x as f32, position.y as f32];
                self.refresh_cursor();
                if let (Some(shop), Some(window)) = (&mut self.shop, &self.window) {
                    let size = window.inner_size();
                    let viewport = [size.width as f32, size.height as f32];
                    shop.hover = shop.item_at(viewport, self.cursor);
                    shop.repair_hover = shop::repair_button_at(viewport, self.cursor);
                    shop.scroll_hover = shop::scroll_direction_at(viewport, self.cursor);
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let amount = match delta {
                    MouseScrollDelta::LineDelta(_, vertical) => vertical,
                    MouseScrollDelta::PixelDelta(position) => position.y as f32,
                };
                if amount != 0.0 && self.scroll_shop_page(amount < 0.0) {
                    return;
                }
                if amount != 0.0 && self.scroll_warehouse_page(amount < 0.0) {
                    return;
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::Focused(false) => {
                self.held = false;
                self.queued_jump = None;
                self.chat_focused = false;
                self.player.stop();
            }
            WindowEvent::KeyboardInput { event, .. }
                if self.warehouse_amount_focused || self.player_shop_price_focused =>
            {
                if event.state == ElementState::Pressed {
                    match event.physical_key {
                        PhysicalKey::Code(
                            KeyCode::Escape | KeyCode::Enter | KeyCode::NumpadEnter,
                        ) => {
                            self.warehouse_amount_focused = false;
                            self.player_shop_price_focused = false;
                        }
                        PhysicalKey::Code(KeyCode::Backspace) => {
                            if self.warehouse_amount_focused {
                                self.warehouse_amount.pop();
                            } else {
                                self.player_shop_price.pop();
                            }
                        }
                        _ => {
                            if let Some(text) = event.text.as_ref() {
                                for ch in text.chars().filter(char::is_ascii_digit) {
                                    if self.warehouse_amount_focused {
                                        if self.warehouse_amount.len() < 10 {
                                            self.warehouse_amount.push(ch);
                                        }
                                    } else if self.player_shop_price.len() < 10 {
                                        self.player_shop_price.push(ch);
                                    }
                                }
                            }
                        }
                    }
                }
                return;
            }
            WindowEvent::KeyboardInput { event, .. }
                if self.dialog.as_ref().is_some_and(|dialog| dialog.input_focused) =>
            {
                if event.state == ElementState::Pressed {
                    match event.physical_key {
                        PhysicalKey::Code(KeyCode::Escape) => {
                            if let Some(dialog) = self.dialog.as_mut() {
                                dialog.input_focused = false;
                                dialog.refresh_edit_row();
                            }
                        }
                        PhysicalKey::Code(KeyCode::Backspace) => {
                            if let Some(dialog) = self.dialog.as_mut() {
                                dialog.input.pop();
                                dialog.refresh_edit_row();
                            }
                        }
                        PhysicalKey::Code(KeyCode::Enter | KeyCode::NumpadEnter) => {
                            self.submit_dialog_text();
                        }
                        _ => {
                            if let (Some(dialog), Some(text)) = (self.dialog.as_mut(), event.text.as_ref()) {
                                for character in text.chars().filter(|character| !character.is_control()) {
                                    if dialog.input.chars().count() < 16 {
                                        dialog.input.push(character);
                                    }
                                }
                                dialog.refresh_edit_row();
                            }
                        }
                    }
                }
                return;
            }
            WindowEvent::KeyboardInput { event, .. } if self.chat_focused => {
                if event.state == ElementState::Pressed {
                    if self.modifiers.control_key()
                        && matches!(event.physical_key, PhysicalKey::Code(KeyCode::KeyV))
                    {
                        if let Ok(mut clipboard) = arboard::Clipboard::new()
                            && let Ok(text) = clipboard.get_text()
                        {
                            for character in
                                text.chars().filter(|character| !character.is_control())
                            {
                                if self.chat_text.chars().count() >= 120 {
                                    break;
                                }
                                self.chat_text.push(character);
                            }
                        }
                        return;
                    }
                    match event.physical_key {
                        PhysicalKey::Code(KeyCode::Escape) => self.chat_focused = false,
                        PhysicalKey::Code(KeyCode::Backspace) => {
                            self.chat_text.pop();
                        }
                        PhysicalKey::Code(KeyCode::Enter | KeyCode::NumpadEnter) => {
                            self.submit_chat();
                        }
                        _ => {
                            if let Some(text) = event.text.as_ref() {
                                for character in
                                    text.chars().filter(|character| !character.is_control())
                                {
                                    if self.chat_text.chars().count() < 120 {
                                        self.chat_text.push(character);
                                    }
                                }
                            }
                        }
                    }
                }
                return;
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if button == MouseButton::Left {
                    debug_log::write(format_args!(
                        "mouse {state:?} cursor=({:.1},{:.1}) inventory_open={} shop={} warehouse={} dragging_inventory={:?} dragging_shop={:?}",
                        self.cursor[0],
                        self.cursor[1],
                        self.inventory_open,
                        self.shop.is_some(),
                        self.warehouse.is_some(),
                        self.dragging_inventory_item
                            .map(|item| (item.uid, item.position)),
                        self.dragging_shop_item,
                    ));
                }
                if button == MouseButton::Left
                    && state == ElementState::Pressed
                    && self.warehouse_amount_focused
                {
                    let inside_amount = self.window.as_ref().is_some_and(|window| {
                        let size = window.inner_size();
                        shop::warehouse_amount_at(
                            [size.width as f32, size.height as f32],
                            self.cursor,
                        )
                    });
                    if !inside_amount {
                        self.warehouse_amount_focused = false;
                    }
                    let inside_price = self.window.as_ref().is_some_and(|window| {
                        let size = window.inner_size();
                        self.quick_panel == Some(shop::QuickPanel::Market)
                            && shop::player_shop_price_at(
                                [size.width as f32, size.height as f32],
                                self.cursor,
                            )
                    });
                    if !inside_price {
                        self.player_shop_price_focused = false;
                    }
                }
                // A release can be consumed by a UI drag below, so clear the
                // world-movement hold before any early return.
                if button == MouseButton::Left && state == ElementState::Released {
                    self.held = false;
                }
                if button == MouseButton::Right && state == ElementState::Pressed {
                    let handled = self.window.as_ref().is_some_and(|window| {
                        let size = window.inner_size();
                        let viewport = [size.width as f32, size.height as f32];
                        if let Some(slot) = hud::action_slot_at(viewport, self.cursor) {
                            self.action_context_slot = Some(slot);
                            true
                        } else if self.action_context_slot.take().is_some() {
                            true
                        } else {
                            false
                        }
                    });
                    if handled {
                        return;
                    }
                }
                if button == MouseButton::Right && state == ElementState::Pressed {
                    if self.begin_item_context_menu() {
                        return;
                    }
                    if self.cast_active_skill_at_cursor() {
                        return;
                    }
                }
                if button == MouseButton::Left && state == ElementState::Pressed {
                    if self.handle_gem_selection_panel_click() {
                        return;
                    }
                    if self.handle_item_context_click() {
                        return;
                    }
                    if self.handle_item_action_panel_click() {
                        return;
                    }
                    if self.handle_inventory_double_click() || self.handle_equipment_double_click()
                    {
                        return;
                    }
                }
                if button == MouseButton::Left && state == ElementState::Pressed {
                    if let Some(slot) = self.action_context_slot {
                        let option = self.window.as_ref().and_then(|window| {
                            let size = window.inner_size();
                            hud::action_context_option_at(
                                [size.width as f32, size.height as f32],
                                self.cursor,
                                slot,
                            )
                        });
                        self.action_context_slot = None;
                        if let Some(option) = option {
                            let labels = [
                                "Assign Spell",
                                "Assign Object",
                                "Assign Text",
                                "Assign Hotkey",
                            ];
                            self.network_status =
                                format!("action bar slot {}: {}", slot + 1, labels[option]);
                        }
                        return;
                    }
                }
                match (button, state) {
                    (MouseButton::Right, ElementState::Pressed)
                        if self.warehouse_item_at_cursor()
                            || (self.warehouse.is_some()
                                && self.deposit_inventory_item_at_cursor())
                            || self.buy_shop_item_at_cursor()
                            || self.sell_inventory_item_at_cursor() =>
                    {
                        return;
                    }
                    (MouseButton::Left, ElementState::Pressed) if self.begin_equipment_drag() => {
                        return;
                    }
                    (MouseButton::Left, ElementState::Pressed) if self.begin_inventory_drag() => {
                        return;
                    }
                    (MouseButton::Left, ElementState::Pressed)
                        if self.close_warehouse_at_cursor() =>
                    {
                        return;
                    }
                    (MouseButton::Left, ElementState::Pressed)
                        if self.handle_warehouse_panel_click() =>
                    {
                        return;
                    }
                    (MouseButton::Left, ElementState::Pressed) if self.select_shop() => return,
                    (MouseButton::Left, ElementState::Released) if self.finish_shop_drag() => {
                        return;
                    }
                    (MouseButton::Left, ElementState::Released) if self.finish_equipment_drag() => {
                        return;
                    }
                    (MouseButton::Left, ElementState::Released) if self.finish_inventory_drag() => {
                        return;
                    }
                    (MouseButton::Left, ElementState::Pressed) if self.select_dialog_option() => {
                        return;
                    }
                    (MouseButton::Left, ElementState::Pressed) if self.handle_hud_click() => {
                        return;
                    }
                    (MouseButton::Left, _) => {}
                    _ => return,
                }
                if state == ElementState::Released {
                    return;
                }
                if self.renderer.as_mut().is_some_and(|renderer| {
                    renderer.minimap_pointer(self.cursor, state == ElementState::Pressed)
                }) {
                    self.held = false;
                    return;
                }
                self.held = state == ElementState::Pressed;
                if self.held && self.pickup_ground_loot_at_cursor() {
                    return;
                }
                if self.held && !self.select_actor_at_cursor() {
                    self.move_cursor();
                }
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::Escape) if self.gem_selection_panel.is_some() => {
                        self.gem_selection_panel = None;
                        self.network_status = "seleção de gem cancelada".into();
                    }
                    PhysicalKey::Code(KeyCode::Escape) if self.item_action_panel.is_some() => {
                        self.item_action_panel = None;
                        self.action_animation_started = None;
                    }
                    PhysicalKey::Code(KeyCode::Escape)
                        if self.player_menu.is_some() || self.pk_menu_open =>
                    {
                        self.player_menu = None;
                        self.pk_menu_open = false;
                    }
                    PhysicalKey::Code(KeyCode::Escape) if self.target_pick.is_some() => {
                        self.target_pick = None;
                        self.network_status = "seleção de alvo cancelada".into();
                    }
                    PhysicalKey::Code(KeyCode::Escape) if self.item_context.is_some() => {
                        self.item_context = None;
                    }
                    PhysicalKey::Code(KeyCode::Escape) if self.shop.is_some() => {
                        self.shop = None;
                    }
                    PhysicalKey::Code(KeyCode::Escape) if self.warehouse.is_some() => {
                        self.warehouse = None;
                        self.warehouse_npc_id = None;
                        self.guild_warehouse_open = false;
                        self.selected_inventory_uid = None;
                        self.selected_warehouse_uid = None;
                    }
                    PhysicalKey::Code(KeyCode::Escape) if self.dialog.is_some() => {
                        self.dialog = None
                    }
                    PhysicalKey::Code(KeyCode::Escape) if self.status_open => {
                        self.status_open = false;
                        self.inventory_open = false;
                    }
                    PhysicalKey::Code(KeyCode::Escape) if self.inventory_open => {
                        self.inventory_open = false;
                    }
                    PhysicalKey::Code(KeyCode::Escape) if self.quick_panel.is_some() => {
                        self.quick_panel = None;
                    }
                    PhysicalKey::Code(KeyCode::Escape) => event_loop.exit(),
                    PhysicalKey::Code(KeyCode::F1) => self.cast_action_slot(0),
                    PhysicalKey::Code(KeyCode::F2) => self.cast_action_slot(1),
                    PhysicalKey::Code(KeyCode::F3) => self.cast_action_slot(2),
                    PhysicalKey::Code(KeyCode::F11) => {
                        self.performance_overlay = !self.performance_overlay;
                    }
                    PhysicalKey::Code(KeyCode::Space) => {
                        self.pending_attack = None;
                        self.pending_skill_attack = None;
                        self.player.stop();
                    }
                    PhysicalKey::Code(KeyCode::Slash) => self.player.running = !self.player.running,
                    PhysicalKey::Code(KeyCode::KeyC) => self.send_network_action(120),
                    PhysicalKey::Code(KeyCode::KeyX) => self.toggle_sit(),
                    PhysicalKey::Code(KeyCode::KeyM) => self.request_mining(),
                    _ => {}
                }
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let previous_cell = self.player.cell();
                self.player
                    .tick(now.duration_since(self.last).as_secs_f32());
                self.last = now;
                let elapsed = self.started.elapsed().as_secs_f32();
                self.send_network_step(previous_cell);
                self.check_portal();
                let expired_dead = self
                    .remote_actors
                    .iter()
                    .filter(|(_, actor)| actor.action == 160 && elapsed >= actor.animation_until)
                    .map(|(id, _)| *id)
                    .collect::<Vec<_>>();
                for id in expired_dead {
                    self.remote_actors.remove(&id);
                    self.remote_monsters.remove(&id);
                    self.remote_models.remove(&id);
                }
                self.process_pending_attack();
                self.process_pending_skill_attack();
                self.process_pending_pickup();
                let mut local_action_progress = None;
                if let Some((action, started_at, until)) = self.local_action {
                    if elapsed < until && !self.player.moving() {
                        self.player.action = action;
                        local_action_progress = Some(
                            ((elapsed - started_at) / (until - started_at).max(f32::EPSILON))
                                .clamp(0.0, 1.0),
                        );
                    } else if elapsed >= until {
                        self.local_action = None;
                    }
                }
                if self.sitting && self.player.moving() {
                    self.sitting = false;
                }
                if self.sitting && self.dead_since.is_none() {
                    self.player.action = Action::Sit;
                    local_action_progress = Some(1.0);
                }
                if let Some(died_at) = self.dead_since {
                    let fall = self.character.duration(Action::Die).as_secs_f32().max(0.1);
                    self.player.action = Action::Die;
                    local_action_progress = Some(((elapsed - died_at) / fall).clamp(0.0, 1.0));
                    if elapsed - died_at >= REVIVE_DELAY_SECONDS && self.dialog.is_none() {
                        self.dialog = Some(DialogState {
                            task_id: REVIVE_TASK_ID,
                            avatar: 0,
                            lines: vec!["You are dead.".to_owned()],
                            options: vec![(1, "Revive".to_owned())],
                            edit: None,
                            input: String::new(),
                            input_focused: false,
                        });
                    }
                }
                let action_animation_progress = self
                    .action_animation_started
                    .map(|started| {
                        let progress = now.duration_since(started).as_secs_f32() / 1.15;
                        if progress >= 1.0 {
                            self.action_animation_started = None;
                            None
                        } else {
                            Some(progress.clamp(0.0, 1.0))
                        }
                    })
                    .flatten();
                self.update_follow();
                if self.target_pick.is_some()
                    != self
                        .shown_cursor
                        .is_some_and(|cursor| !matches!(cursor, cursors::GameCursor::Arrow | cursors::GameCursor::Interact))
                {
                    self.refresh_cursor();
                }
                if !self.player.moving()
                    && self.pending_network_steps.is_empty()
                    && let Some(target) = self.queued_jump.take()
                {
                    self.start_jump(target);
                } else if self.held && !self.player.moving() {
                    self.move_cursor();
                }
                if let Some(audio) = &mut self.audio {
                    audio.update(&self.player);
                }
                let skill_hud = self.skill_hud();
                let choice_menu = self.choice_menu();
                let notice = self.target_pick.map(TargetPick::notice);
                let prompt = self.team_prompt();
                let hovered_player = self.hovered_player();
                if let Some(renderer) = &mut self.renderer
                    && let Err(error) = renderer.draw(
                        &mut self.assets,
                        &self.map,
                        &self.character,
                        &self.action_bar,
                        &self.known_skills,
                        &skill_hud,
                        &self.team_members,
                        self.team_leader_id,
                        self.pending_team_invite,
                        self.pending_team_join_request,
                        self.team_join_enabled,
                        &self.guild_members,
                        self.pending_guild_invite,
                        &self.trade,
                        self.action_context_slot,
                        &self.player,
                        &self.remote_actors,
                        self.selected_target,
                        &self.remote_models,
                        &self.remote_npcs,
                        &self.remote_monsters,
                        &self.combat_numbers,
                        &self.skill_effects,
                        &self.ground_loot,
                        self.hit_until,
                        self.level_up_until,
                        self.dialog
                            .as_ref()
                            .map(|dialog| {
                                (dialog.lines.as_slice(), dialog.options.as_slice(), dialog.avatar)
                            }),
                        self.shop.as_ref(),
                        &self.equipment,
                        &self.hidden_default_equipment_slots,
                        &self.inventory,
                        self.inventory_open,
                        self.inventory_filter,
                        self.selected_inventory_uid,
                        self.warehouse.as_deref(),
                        self.guild_warehouse_open,
                        self.warehouse_page,
                        self.selected_warehouse_uid,
                        self.warehouse_silver,
                        &self.warehouse_amount,
                        self.warehouse_amount_focused,
                        &self.chat_text,
                        self.chat_focused,
                        self.chat_history.make_contiguous(),
                        self.chat_filter,
                        &self.network_status,
                        &self.friends,
                        &self.ignores,
                        self.pending_friend_request.as_ref().map(|(_, name)| name.as_str()),
                        self.silver,
                        self.cps,
                        self.mana,
                        self.maximum_mana,
                        self.level,
                        self.experience,
                        self.nobility_donation,
                        self.nobility_rank,
                        self.nobility_remaining,
                        self.equipped_title,
                        &self.nobility_ranking,
                        self.status_flags,
                        self.status_open,
                        self.status_tab,
                        self.performance_overlay,
                        self.quick_panel,
                        self.hud_config,
                        self.network_character_id,
                        &self.player_shop_items,
                        self.player_shop_open,
                        &self.player_shop_price,
                        self.player_shop_price_focused,
                        self.item_context.clone(),
                        choice_menu,
                        notice,
                        prompt,
                        hovered_player,
                        self.item_action_panel,
                        action_animation_progress,
                        self.gem_selection_panel,
                        self.dragging_shop_item,
                        self.dragging_inventory_item,
                        self.cursor,
                        local_action_progress,
                        self.started.elapsed().as_secs_f32(),
                    )
                {
                    self.error = Some(error.to_string());
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }
    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        let events = self
            .network
            .as_ref()
            .map(NetworkHandle::drain)
            .unwrap_or_default();
        for event in events {
            self.handle_network_event(event);
        }
        self.poll_remote_model_loader();
        // Interpolate remote movement between server walk packets. The server
        // sends tile steps; rendering the whole step immediately makes mobs
        // look like they teleport or freeze between updates.
        let now = self.started.elapsed().as_secs_f32();
        self.combat_numbers.retain(|(_, _, expires)| *expires > now);
        self.skill_effects.retain(|effect| effect.expires > now);
        for slot in &mut self.action_bar.slots {
            let skill_id = slot
                .action_id
                .as_deref()
                .and_then(|action| action.strip_prefix("skill:"))
                .and_then(|id| id.parse::<u32>().ok());
            let remaining = skill_id
                .and_then(|skill_id| self.skill_cooldown_until.get(&skill_id).copied())
                .map_or(0.0, |until| (until - now).max(0.0));
            let total = skill_id
                .and_then(action_bar::skill_action)
                .map_or(1.0, |skill| skill.cooldown_seconds);
            // Whole seconds only for long cooldowns; the sweep shows the rest.
            slot.cooldown = Some(remaining.ceil() as u32).filter(|seconds| *seconds > 1);
            slot.cooldown_left = action_bar::cooldown_steps(remaining, total);
        }
        self.ground_loot.retain(|(_, _, _, expires)| *expires > now);
        if let Some(character_id) = self.network_character_id {
            let hud = settings::HudSettings::capture(
                &self.action_bar,
                self.skill_bar_open,
                self.performance_overlay,
                self.team_join_enabled,
            );
            if self.saved_hud.as_ref() != Some(&hud) {
                settings::save(character_id, &hud);
                self.saved_hud = Some(hud);
            }
        }
        for actor in self.remote_actors.values_mut() {
            if actor.movement_until > actor.movement_started && now < actor.movement_until {
                let t = ((now - actor.movement_started)
                    / (actor.movement_until - actor.movement_started))
                    .clamp(0.0, 1.0);
                actor.position[0] =
                    actor.movement_from[0] + (actor.movement_to[0] - actor.movement_from[0]) * t;
                actor.position[1] =
                    actor.movement_from[1] + (actor.movement_to[1] - actor.movement_from[1]) * t;
            } else if actor.movement_until > actor.movement_started {
                actor.position = actor.movement_to;
            }
        }
        if let Some(window) = &self.window {
            let now = Instant::now();
            if now >= self.next_frame_at {
                window.request_redraw();
                self.next_frame_at = self.frame_interval.map_or(now, |interval| now + interval);
            }
            if self.frame_interval.is_some() {
                // A full `WaitUntil` can wake late on Windows, while a full
                // `Poll` wastes a core at every fixed cap. Wake a small
                // margin before the deadline, then poll only the final slice
                // so 60..500 FPS remain accurate without busy-spinning for
                // the entire frame interval.
                let precision_margin = std::time::Duration::from_millis(2);
                let precision_deadline = self
                    .next_frame_at
                    .checked_sub(precision_margin)
                    .unwrap_or(now);
                if now >= precision_deadline {
                    _event_loop.set_control_flow(ControlFlow::Poll);
                } else {
                    _event_loop.set_control_flow(ControlFlow::WaitUntil(precision_deadline));
                }
            } else {
                _event_loop.set_control_flow(ControlFlow::Poll);
            }
        }
    }
}
impl App {
    fn poll_remote_model_loader(&mut self) {
        let Some(loader) = &self.remote_model_loader else {
            return;
        };
        let results = loader.results.try_iter().collect::<Vec<_>>();
        for result in results {
            match result {
                RemoteModelResult::Player { id, model } => {
                    self.remote_models_loading.remove(&id);
                    match model {
                        Ok(model) => {
                            if self.network_character_id == Some(id) {
                                let mut model = model;
                                model.guild = self.character.guild.clone();
                                model.guild_rank = self.character.guild_rank.clone();
                                model.maximum_health =
                                    self.character.maximum_health.max(model.maximum_health);
                                self.character = model;
                                self.renderer
                                    .as_mut()
                                    .map(|renderer| renderer.invalidate_status());
                            } else {
                                self.remote_models.insert(id, model);
                            }
                        }
                        Err(error) => eprintln!("Player {id} model unavailable: {error}"),
                    }
                }
                RemoteModelResult::Npc { id, model } => {
                    self.remote_npcs_loading.remove(&id);
                    match model {
                        Ok(model) => {
                            self.remote_npcs.insert(id, model);
                        }
                        Err(error) => eprintln!("NPC {id} model unavailable: {error}"),
                    }
                }
                RemoteModelResult::Monster { id, model } => {
                    self.remote_monsters_loading.remove(&id);
                    match model {
                        Ok(model) => {
                            self.remote_monsters.insert(id, model);
                        }
                        Err(error) => eprintln!("Monster {id} model unavailable: {error}"),
                    }
                }
            }
        }
    }

    fn hide_default_equipment_slot(&mut self, slot: u8) {
        if let Some(hidden) = self
            .hidden_default_equipment_slots
            .get_mut(usize::from(slot))
        {
            *hidden = true;
        }
    }

    /// Garments are represented by the classic armor model catalog.  The
    /// server keeps the normal armor and garment as separate equipment
    /// entries, but the client can render only one body outfit at a time, so
    /// the garment must win while it is equipped.
    fn appearance_armor(&self, fallback: u32) -> u32 {
        self.equipment
            .iter()
            .find(|item| (180..=189).contains(&(item.item_type / 1_000)))
            .map(|item| item.item_type)
            .unwrap_or(fallback)
    }

    fn refresh_character_equipment_model(&mut self) {
        let armor = self
            .equipment
            .iter()
            .find(|item| item.position == 3)
            .map(|item| character::dyed(item.item_type, item.color))
            .unwrap_or(0);
        let armor = self.appearance_armor(armor);
        let helmet = self
            .equipment
            .iter()
            .find(|item| item.position == 1)
            .map(|item| character::dyed(item.item_type, item.color))
            .unwrap_or(0);
        let right_weapon = self
            .equipment
            .iter()
            .find(|item| item.position == 4)
            .map(|item| item.item_type)
            .unwrap_or(0);
        let left_weapon = self
            .equipment
            .iter()
            .find(|item| item.position == 5)
            .map(|item| character::dyed(item.item_type, item.color))
            .unwrap_or(0);
        debug_log::write(format_args!(
            "refresh world model: helmet={} armor_or_garment={} right_weapon={} left_weapon={} equipment_count={}",
            helmet,
            armor,
            right_weapon,
            left_weapon,
            self.equipment.len()
        ));

        // Equipment packets can arrive in a burst during login or after a
        // drag-and-drop action. Model/C3 decoding is expensive and must not
        // block the winit event thread; the worker result is applied by
        // `poll_remote_model_loader` in the same FIFO order as the packets.
        if let (Some(id), Some(loader)) = (self.network_character_id, &self.remote_model_loader) {
            let job = RemoteModelJob::Player {
                id,
                body: self.character.body_id,
                armor,
                helmet,
                hair: self.character.hair_id as u16,
                right_weapon,
                left_weapon,
                name: self.character.name.clone(),
                health: self.character.health.min(u16::MAX as u32) as u16,
            };
            if loader.requests.send(job).is_err() {
                eprintln!("Local character equipment model update could not be queued");
            }
            return;
        }

        // Offline previews have no network character ID/loader, so retain the
        // direct path used by the local-only sandbox.
        let Ok(mut character) = character::Character::load_remote(
            &mut self.assets,
            self.character.body_id,
            armor,
            helmet,
            self.character.hair_id as u16,
            right_weapon,
            left_weapon,
            self.character.name.clone(),
            self.character.health.min(u16::MAX as u32) as u16,
        ) else {
            return;
        };
        character.guild = self.character.guild.clone();
        character.guild_rank = self.character.guild_rank.clone();
        character.maximum_health = self.character.maximum_health;
        self.character = character;
        if let Some(renderer) = &mut self.renderer {
            renderer.invalidate_status();
        }
    }

    fn handle_hud_click(&mut self) -> bool {
        let Some(window) = &self.window else {
            return false;
        };
        let size = window.inner_size();
        let viewport = [size.width as f32, size.height as f32];
        if let Some(slot) = hud::action_slot_at(viewport, self.cursor) {
            self.cast_action_slot(slot);
            self.held = false;
            return true;
        }
        if let Some(prompt) = self.team_prompt() {
            if let Some(accept) = prompt.button_at(self.cursor) {
                self.answer_team_prompt(accept);
                self.held = false;
                return true;
            }
            if prompt.contains(self.cursor) {
                self.held = false;
                return true;
            }
        }
        if let Some(player_menu) = self.player_menu.take() {
            if let Some(row) = player_menu.menu.row_at(self.cursor) {
                self.player_menu_action(player_menu.target_id, &player_menu.name, PLAYER_MENU[row].1);
                self.held = false;
                return true;
            }
            if player_menu.menu.contains(self.cursor) {
                self.player_menu = Some(player_menu);
                self.held = false;
                return true;
            }
            // Any click elsewhere closes the menu and is handled as usual.
        }
        if self.pk_menu_open {
            let menu = shop::ChoiceMenu::kill_modes(viewport, self.kill_mode);
            if let Some(mode) = menu.row_at(self.cursor) {
                self.pk_menu_open = false;
                self.set_kill_mode(mode as u8);
                self.held = false;
                return true;
            }
            if menu.contains(self.cursor) {
                self.held = false;
                return true;
            }
            // Any click elsewhere closes the menu and is handled as usual.
            self.pk_menu_open = false;
            if hud::capture_button_at(viewport, self.cursor) {
                self.held = false;
                return true;
            }
        } else if hud::capture_button_at(viewport, self.cursor) {
            self.pk_menu_open = true;
            self.chat_focused = false;
            self.held = false;
            return true;
        }
        let skill_hud = self.skill_hud();
        if let Some(skill_id) = hud::skill_bar_at(&mut self.assets, viewport, self.cursor, &skill_hud)
        {
            self.active_skill = Some(skill_id);
            self.skill_bar_open = false;
            self.network_status = format!("skill selecionada: {skill_id}");
            self.held = false;
            return true;
        }
        if self.quick_panel.is_some() {
            if self.quick_panel == Some(shop::QuickPanel::Settings)
                && let Some(action) = shop::settings_panel_action_at(viewport, self.cursor)
            {
                match action {
                    shop::SettingsPanelAction::ToggleArcs => {
                        self.hud_config.center_arcs = !self.hud_config.center_arcs;
                    }
                    shop::SettingsPanelAction::ToggleBars => {
                        self.hud_config.center_bars = !self.hud_config.center_bars;
                    }
                    shop::SettingsPanelAction::ScaleDown => {
                        self.hud_config.center_scale =
                            self.hud_config.center_scale.saturating_sub(10).max(75);
                    }
                    shop::SettingsPanelAction::ScaleUp => {
                        self.hud_config.center_scale = (self.hud_config.center_scale + 10).min(140);
                    }
                    shop::SettingsPanelAction::DistanceDown => {
                        self.hud_config.center_arc_distance = self
                            .hud_config
                            .center_arc_distance
                            .saturating_sub(10)
                            .max(80);
                    }
                    shop::SettingsPanelAction::DistanceUp => {
                        self.hud_config.center_arc_distance =
                            (self.hud_config.center_arc_distance + 10).min(180);
                    }
                }
                self.network_status = "configuração do HUD atualizada".into();
                self.held = false;
                return true;
            }
            if self.quick_panel == Some(shop::QuickPanel::Market)
                && shop::player_shop_price_at(viewport, self.cursor)
            {
                self.warehouse_amount_focused = false;
                self.player_shop_price_focused = true;
                self.held = false;
                return true;
            }
            if self
                .quick_panel
                .is_some_and(|panel| shop::quick_panel_close_at(viewport, self.cursor, panel))
            {
                self.quick_panel = None;
                self.player_shop_price_focused = false;
                self.held = false;
                return true;
            }
            if self.quick_panel == Some(shop::QuickPanel::Team) {
                if let Some(action) = shop::team_panel_action_at(
                    viewport,
                    self.cursor,
                    !self.team_members.is_empty(),
                    self.pending_team_invite.is_some(),
                    self.pending_team_join_request.is_some(),
                ) {
                    self.handle_team_panel_action(action);
                    self.held = false;
                    return true;
                }
                if let Some(member_id) =
                    shop::team_panel_member_at(viewport, self.cursor, &self.team_members)
                {
                    self.selected_target = Some(member_id);
                    self.network_status = format!("team target selected: {member_id}");
                    self.held = false;
                    return true;
                }
            }
            if self.quick_panel == Some(shop::QuickPanel::Guild) {
                let has_guild = !self.character.guild.is_empty() || !self.guild_members.is_empty();
                if let Some(action) = shop::guild_panel_action_at(
                    viewport,
                    self.cursor,
                    has_guild,
                    self.pending_guild_invite.is_some(),
                ) {
                    self.handle_guild_panel_action(action);
                    self.held = false;
                    return true;
                }
                if let Some(member_id) =
                    shop::guild_panel_member_at(viewport, self.cursor, &self.guild_members)
                {
                    self.selected_target = Some(member_id);
                    self.network_status = format!("guild target selected: {member_id}");
                    self.held = false;
                    return true;
                }
            }
            if self.quick_panel == Some(shop::QuickPanel::Friends) {
                if let Some(action) = shop::friends_panel_action_at(
                    viewport,
                    self.cursor,
                    &self.friends,
                    &self.ignores,
                    self.pending_friend_request.is_some(),
                ) {
                    self.handle_friends_panel_action(action);
                    self.held = false;
                    return true;
                }
            }
            if self.quick_panel == Some(shop::QuickPanel::Trade) {
                if let Some(action) = shop::trade_panel_action_at(viewport, self.cursor, &self.trade) {
                    self.handle_trade_panel_action(action);
                    self.held = false;
                    return true;
                }
            }
            if self.quick_panel == Some(shop::QuickPanel::Market) {
                let has_selected_target = self.selected_player_target().is_some();
                if let Some(action) = shop::player_shop_panel_action_at(
                    viewport,
                    self.cursor,
                    self.player_shop_open,
                    &self.player_shop_items,
                    self.network_character_id,
                    has_selected_target,
                ) {
                    self.handle_player_shop_panel_action(action);
                    self.held = false;
                    return true;
                }
            }
            if self
                .quick_panel
                .is_some_and(|panel| shop::quick_panel_at(viewport, self.cursor, panel))
            {
                self.held = false;
                return true;
            }
        }
        if self.status_open {
            if shop::status_close_at(viewport, self.cursor) {
                self.status_open = false;
                self.inventory_open = false;
                self.held = false;
                return true;
            }
            if self.status_tab == shop::StatusTab::Attributes
                && self.character.attribute_points > 0
                && let Some(attribute) = shop::status_attribute_button_at(viewport, self.cursor)
            {
                if let Some(network) = &self.network {
                    network.assign_attribute(attribute);
                    self.network_status = format!("online: allocating {attribute}");
                }
                self.held = false;
                return true;
            }
            if self.status_tab == shop::StatusTab::Reputation
                && shop::status_reputation_donate_at(viewport, self.cursor)
            {
                if let (Some(network), Some(character_id)) =
                    (&self.network, self.network_character_id)
                {
                    network.send_chat(
                        character_id,
                        &self.character.name,
                        "$nobility-donate 3000000",
                    );
                    self.network_status = "online: doação de 3.000.000 silver enviada".into();
                } else {
                    self.network_status = "doação requer conexão online".into();
                }
                self.held = false;
                return true;
            }
            if self.status_tab == shop::StatusTab::Title
                && let Some(title_id) = shop::status_title_at(viewport, self.cursor)
            {
                if let (Some(network), Some(character_id)) =
                    (&self.network, self.network_character_id)
                {
                    let command = format!("$title-equip {title_id}");
                    network.send_chat(character_id, &self.character.name, &command);
                    self.network_status = if title_id == 0 {
                        "online: removendo título equipado".into()
                    } else {
                        format!("online: equipando título {title_id}")
                    };
                } else {
                    self.network_status = "títulos requerem conexão online".into();
                }
                self.held = false;
                return true;
            }
            if self.status_tab == shop::StatusTab::Weapon
                && let Some(filter) = shop::status_weapon_filter_at(viewport, self.cursor)
            {
                self.weapon_filter = filter;
                self.held = false;
                return true;
            }
            if let Some(tab) = shop::status_tab_at(viewport, self.cursor) {
                if tab == shop::StatusTab::Weapon && self.status_tab != tab {
                    // Open on the page of the weapon in hand.
                    if let Some(weapon) = self.equipment.iter().find(|item| item.position == 4) {
                        self.weapon_filter = shop::WeaponFilter::of(weapon.item_type / 1_000);
                    }
                }
                self.status_tab = tab;
                self.held = false;
                return true;
            }
            if shop::status_panel_at(viewport, self.cursor) {
                self.held = false;
                return true;
            }
        }
        if let Some(button) = hud::quick_button_at(viewport, self.cursor) {
            match button {
                0 => {
                    self.status_open = !self.status_open;
                    self.inventory_open = false;
                    self.quick_panel = None;
                }
                // Trade, as in the original client, turns the cursor into a
                // trade request: the next player clicked is asked. An open
                // or requested trade shows its panel instead.
                6 if !self.trade.active
                    && self.trade.pending_request.is_none()
                    && self.quick_panel != Some(shop::QuickPanel::Trade) =>
                {
                    if self.target_pick == Some(TargetPick::Trade) {
                        self.target_pick = None;
                        self.network_status = "seleção de alvo cancelada".into();
                    } else {
                        self.target_pick = Some(TargetPick::Trade);
                        self.network_status = "clique no jogador para negociar".into();
                    }
                }
                // "Skill" toggles the bar with the learned skills above the
                // HUD, as in the original client.
                2 => {
                    self.skill_bar_open = !self.skill_bar_open;
                }
                1 => {
                    self.inventory_open = !self.inventory_open;
                    self.quick_panel = None;
                    if !self.inventory_open {
                        self.selected_inventory_uid = None;
                    }
                }
                _ => {
                    self.status_open = false;
                    self.inventory_open = false;
                    self.quick_panel = if self.quick_panel.is_some() {
                        None
                    } else {
                        shop::QuickPanel::from_button(button)
                    };
                }
            }
            self.chat_focused = false;
            self.held = false;
            return true;
        }
        if hud::items_button_at(viewport, self.cursor) {
            self.chat_focused = false;
            self.inventory_open = !self.inventory_open;
            if !self.inventory_open {
                self.selected_inventory_uid = None;
            }
            self.held = false;
            return true;
        }
        if hud::status_button_at(viewport, self.cursor) {
            self.status_open = !self.status_open;
            self.inventory_open = false;
            self.chat_focused = false;
            self.held = false;
            return true;
        }
        if hud::chat_clear_at(viewport, self.cursor) {
            self.chat_history.clear();
            self.network_status = "chat history cleared".into();
            self.chat_focused = false;
            self.held = false;
            return true;
        }
        if hud::chat_send_at(viewport, self.cursor) {
            self.submit_chat();
            self.held = false;
            return true;
        }
        if let Some(filter) = hud::chat_tab_at(viewport, self.cursor) {
            self.chat_filter = filter;
            self.chat_focused = false;
            self.held = false;
            return true;
        }
        if hud::chat_input_at(viewport, self.cursor)
            || hud::chat_button_at(viewport, self.cursor)
            || hud::talk_channel_at(viewport, self.cursor)
        {
            self.chat_focused = true;
            self.held = false;
            return true;
        }
        self.chat_focused = false;
        if hud::contains(viewport, self.cursor) {
            self.held = false;
            return true;
        }
        false
    }

    /// The cursor for what is under the mouse: a target reticle while a
    /// Friend/Trade pick waits, the bright arrow over clickable things, else
    /// the arrow.
    fn wanted_cursor(&self) -> cursors::GameCursor {
        use cursors::GameCursor;
        match self.target_pick {
            Some(TargetPick::Friend) => return GameCursor::PickFriend,
            Some(TargetPick::Ignore) => return GameCursor::PickEnemy,
            Some(TargetPick::Trade) => return GameCursor::PickTrade,
            None => {}
        }
        if self.ground_loot_at_cursor().is_some() {
            return GameCursor::Interact;
        }
        let Some(window) = &self.window else {
            return GameCursor::Arrow;
        };
        let size = window.inner_size();
        let viewport = [size.width as f32, size.height as f32];
        let over_action_bar = hud::action_slot_at(viewport, self.cursor).is_some();
        let over_context = self
            .action_context_slot
            .is_some_and(|slot| hud::action_context_option_at(viewport, self.cursor, slot).is_some());
        let over_menu = self.choice_menu().is_some_and(|menu| menu.row_at(self.cursor).is_some());
        if over_action_bar || over_context || over_menu {
            GameCursor::Interact
        } else {
            GameCursor::Arrow
        }
    }

    fn refresh_cursor(&mut self) {
        let wanted = self.wanted_cursor();
        if self.shown_cursor == Some(wanted) {
            return;
        }
        let Some(window) = &self.window else {
            return;
        };
        // Fall back to the system cursors if ours could not be created.
        match self.cursors.get(&wanted) {
            Some(cursor) => window.set_cursor(cursor.clone()),
            None => window.set_cursor(match wanted {
                cursors::GameCursor::Arrow => CursorIcon::Default,
                cursors::GameCursor::Interact => CursorIcon::Pointer,
                _ => CursorIcon::Crosshair,
            }),
        }
        self.shown_cursor = Some(wanted);
    }

    /// The popup for a received team invite, or for a request to join the
    /// hero's team.
    fn team_prompt(&self) -> Option<shop::PromptDialog> {
        let (id, text) = if let Some(leader) = self.pending_team_invite {
            (leader, "invites you to the team")
        } else {
            (self.pending_team_join_request?, "asks to join your team")
        };
        let size = self.window.as_ref()?.inner_size();
        let name = self.actor_name(id);
        let name = if name.is_empty() { id.to_string() } else { name };
        Some(shop::PromptDialog::new(
            [size.width as f32, size.height as f32],
            "Team",
            name,
            text,
        ))
    }

    fn answer_team_prompt(&mut self, accept: bool) {
        let invite = self.pending_team_invite.is_some();
        match (invite, accept) {
            (true, true) => self.handle_team_panel_action(shop::TeamPanelAction::AcceptInvite),
            (false, true) => self.handle_team_panel_action(shop::TeamPanelAction::AcceptJoin),
            (true, false) => {
                self.pending_team_invite = None;
                self.network_status = "convite de grupo recusado".into();
            }
            (false, false) => {
                self.pending_team_join_request = None;
                self.network_status = "pedido de entrada no grupo recusado".into();
            }
        }
    }

    /// The open choice menu: the kill modes or a player's Shift+click menu.
    fn choice_menu(&self) -> Option<shop::ChoiceMenu> {
        if self.pk_menu_open {
            let size = self.window.as_ref()?.inner_size();
            let viewport = [size.width as f32, size.height as f32];
            return Some(shop::ChoiceMenu::kill_modes(viewport, self.kill_mode));
        }
        self.player_menu.as_ref().map(|player_menu| player_menu.menu.clone())
    }

    fn open_player_menu(&mut self, target_id: u32, name: String) {
        let Some(window) = &self.window else {
            return;
        };
        let size = window.inner_size();
        let viewport = [size.width as f32, size.height as f32];
        let menu = shop::ChoiceMenu::at(
            viewport,
            self.cursor,
            name.clone(),
            PLAYER_MENU.iter().map(|(label, _)| *label).collect(),
        );
        self.pk_menu_open = false;
        self.target_pick = None;
        self.selected_target = Some(target_id);
        self.player_menu = Some(PlayerMenu {
            target_id,
            name,
            menu,
        });
    }

    fn player_menu_action(&mut self, target_id: u32, name: &str, action: PlayerMenuAction) {
        // The handlers below act on the selected player.
        self.selected_target = Some(target_id);
        match action {
            PlayerMenuAction::Trade => self.handle_trade_panel_action(shop::TradePanelAction::Request),
            PlayerMenuAction::AddFriend => {
                self.handle_friends_panel_action(shop::FriendsPanelAction::AddFriend);
            }
            PlayerMenuAction::Enemy => {
                self.handle_friends_panel_action(shop::FriendsPanelAction::AddIgnore);
            }
            PlayerMenuAction::TeamInvite => {
                const REQUEST_INVITE: u32 = 4;
                self.send_team_action(REQUEST_INVITE, target_id);
            }
            PlayerMenuAction::Whisper => {
                self.chat_text = format!("/w {name} ");
                self.chat_focused = true;
                self.network_status = format!("whisper preparado para {name}");
            }
            PlayerMenuAction::Follow => {
                self.follow_target = Some(target_id);
                self.network_status = format!("seguindo {name}");
            }
        }
    }

    /// Walks after the followed player while it is more than two cells away.
    fn update_follow(&mut self) {
        let Some(target_id) = self.follow_target else {
            return;
        };
        let Some(actor) = self.remote_actors.get(&target_id) else {
            self.follow_target = None;
            self.network_status = "alvo seguido saiu da tela".into();
            return;
        };
        if self.player.moving() || !self.pending_network_steps.is_empty() || self.dead_since.is_some() {
            return;
        }
        let target = actor.position.map(|value| value.round() as i32);
        let here = self.player.cell();
        if (target[0] - here[0]).abs().max((target[1] - here[1]).abs()) > 2 {
            self.player.go(&self.map, target);
        }
    }

    fn set_kill_mode(&mut self, mode: u8) {
        let (Some(network), Some(character_id)) = (&self.network, self.network_character_id) else {
            self.network_status = "modo PK requer conexão online".into();
            return;
        };
        // The server answers with the same MsgAction 96 and a notice.
        network.send_kill_mode(character_id, mode);
        self.network_status = format!("online: modo {}", shop::KILL_MODES[usize::from(mode)]);
    }

    fn submit_chat(&mut self) {
        let message = self.chat_text.trim().to_owned();
        if !message.is_empty()
            && let (Some(network), Some(character_id)) = (&self.network, self.network_character_id)
        {
            if let Some(rest) = message
                .strip_prefix("/w ")
                .or_else(|| message.strip_prefix("/whisper "))
            {
                let mut parts = rest.trim().splitn(2, char::is_whitespace);
                let recipient = parts.next().unwrap_or_default().trim();
                let whisper = parts.next().unwrap_or_default().trim();
                if recipient.is_empty() || whisper.is_empty() {
                    self.network_status = "uso: /w <personagem> <mensagem>".into();
                } else {
                    network.send_whisper(character_id, &self.character.name, recipient, whisper);
                    self.network_status = format!("online: whisper enviado para {recipient}");
                }
            } else {
                network.send_chat(character_id, &self.character.name, &message);
            }
        }
        self.chat_text.clear();
    }

    fn send_team_action(&mut self, action: u32, target_id: u32) {
        let Some(network) = &self.network else {
            self.network_status = "team indisponível: servidor offline".into();
            return;
        };
        network.send_team(action, target_id);
        self.network_status = format!("online: team action {action} target {target_id}");
    }

    /// The name of an actor on screen, or empty when it is not known.
    fn actor_name(&self, id: u32) -> String {
        self.remote_actors
            .get(&id)
            .map(|actor| actor.name.clone())
            .unwrap_or_default()
    }

    fn selected_player_target(&self) -> Option<(u32, String)> {
        let target_id = self.selected_target?;
        let actor = self.remote_actors.get(&target_id)?;
        if actor.npc || (400001..=499999).contains(&actor.id) || actor.name.is_empty() {
            return None;
        }
        Some((target_id, actor.name.clone()))
    }

    fn handle_friends_panel_action(&mut self, action: shop::FriendsPanelAction) {
        match action {
            shop::FriendsPanelAction::AddFriend | shop::FriendsPanelAction::AddIgnore => {
                let Some((target_id, target_name)) = self.selected_player_target() else {
                    self.target_pick = Some(if action == shop::FriendsPanelAction::AddFriend {
                        TargetPick::Friend
                    } else {
                        TargetPick::Ignore
                    });
                    self.network_status = "clique no jogador".into();
                    return;
                };
                let action_code = match action {
                    shop::FriendsPanelAction::AddFriend => 10,
                    shop::FriendsPanelAction::AddIgnore => 19,
                    _ => unreachable!(),
                };
                if let Some(network) = &self.network {
                    network.send_friend_action(action_code, target_id, &target_name);
                    self.network_status = if action_code == 10 {
                        format!("online: pedido de amizade enviado para {target_name}")
                    } else {
                        format!("online: {target_name} adicionado aos ignorados")
                    };
                } else {
                    self.network_status = "social indisponível: servidor offline".into();
                }
            }
            shop::FriendsPanelAction::AcceptRequest => {
                let Some((requester_id, name)) = self.pending_friend_request.take() else {
                    return;
                };
                // The server makes the friendship when the requested player
                // applies back to the requester.
                if let Some(network) = &self.network {
                    network.send_friend_action(10, requester_id, &name);
                    self.network_status = format!("online: amizade com {name} aceita");
                } else {
                    self.network_status = "social indisponível: servidor offline".into();
                }
            }
            shop::FriendsPanelAction::RefuseRequest => {
                if let Some((_, name)) = self.pending_friend_request.take() {
                    self.network_status = format!("pedido de amizade de {name} recusado");
                }
            }
            shop::FriendsPanelAction::Whisper(index) => {
                let Some(entry) = self.friends.get(index) else {
                    return;
                };
                self.chat_text = format!("/w {} ", entry.name);
                self.chat_focused = true;
                self.quick_panel = None;
                self.network_status = format!("whisper preparado para {}", entry.name);
            }
            shop::FriendsPanelAction::RemoveFriend(index) => {
                let Some(entry) = self.friends.get(index) else {
                    return;
                };
                let (friend_id, name) = (entry.friend_id, entry.name.clone());
                if let Some(network) = &self.network {
                    network.send_friend_action(14, friend_id, &name);
                    self.network_status = format!("online: removendo amizade com {name}");
                } else {
                    self.network_status = "social indisponível: servidor offline".into();
                }
            }
            shop::FriendsPanelAction::RemoveIgnore(index) => {
                let Some(entry) = self.ignores.get(index) else {
                    return;
                };
                let (friend_id, name) = (entry.friend_id, entry.name.clone());
                if let Some(network) = &self.network {
                    network.send_friend_action(18, friend_id, &name);
                    self.network_status = format!("online: removendo {name} dos ignorados");
                } else {
                    self.network_status = "social indisponível: servidor offline".into();
                }
            }
        }
    }

    fn handle_team_panel_action(&mut self, action: shop::TeamPanelAction) {
        const CREATE: u32 = 0;
        const REQUEST_JOIN: u32 = 1;
        const ACCEPT_INVITE: u32 = 3;
        const ACCEPT_JOIN: u32 = 5;
        const LEAVE_TEAM: u32 = 2;
        const REQUEST_INVITE: u32 = 4;
        const DISMISS: u32 = 6;
        const KICK: u32 = 7;
        const FORBID_NEW_MEMBERS: u32 = 8;
        const ALLOW_NEW_MEMBERS: u32 = 9;
        const LEADER: u32 = 15;
        let target = self.selected_target.unwrap_or_default();
        match action {
            shop::TeamPanelAction::Create => self.send_team_action(CREATE, 0),
            shop::TeamPanelAction::AcceptInvite => {
                let Some(leader_id) = self.pending_team_invite.take() else {
                    self.network_status = "nenhum convite pendente".into();
                    return;
                };
                self.send_team_action(ACCEPT_INVITE, leader_id);
            }
            shop::TeamPanelAction::RequestJoin => {
                if target == 0 {
                    self.network_status =
                        "selecione o líder do grupo para solicitar entrada".into();
                } else {
                    self.send_team_action(REQUEST_JOIN, target);
                }
            }
            shop::TeamPanelAction::AcceptJoin => {
                let Some(requester_id) = self.pending_team_join_request.take() else {
                    self.network_status = "nenhuma solicitação de entrada pendente".into();
                    return;
                };
                self.send_team_action(ACCEPT_JOIN, requester_id);
            }
            shop::TeamPanelAction::Invite => {
                if target == 0 {
                    self.network_status = "selecione um jogador para convidar".into();
                } else {
                    self.send_team_action(REQUEST_INVITE, target);
                }
            }
            shop::TeamPanelAction::Leave => self.send_team_action(LEAVE_TEAM, 0),
            shop::TeamPanelAction::Leader => {
                if target == 0 {
                    self.network_status = "selecione um membro para passar a liderança".into();
                } else {
                    self.send_team_action(LEADER, target);
                }
            }
            shop::TeamPanelAction::Kick => {
                if target == 0 {
                    self.network_status = "selecione um membro para expulsar".into();
                } else {
                    self.send_team_action(KICK, target);
                }
            }
            shop::TeamPanelAction::Dismiss => self.send_team_action(DISMISS, 0),
            shop::TeamPanelAction::ToggleJoin => {
                let action = if self.team_join_enabled {
                    FORBID_NEW_MEMBERS
                } else {
                    ALLOW_NEW_MEMBERS
                };
                self.send_team_action(action, 0);
            }
        }
    }

    fn handle_guild_panel_action(&mut self, action: shop::GuildPanelAction) {
        const INVITE_JOIN: u32 = 2;
        const LEAVE_GUILD: u32 = 3;
        const KICK_MEMBER: u32 = 4;
        match action {
            shop::GuildPanelAction::AcceptInvite => {
                if self.pending_guild_invite.take().is_some() {
                    self.network
                        .as_ref()
                        .map(|network| network.send_guild_action(INVITE_JOIN, 0));
                    self.network_status = "online: aceitando convite da guild".into();
                }
            }
            shop::GuildPanelAction::Warehouse => {
                let Some(guild_id) = self.guild_id else {
                    self.network_status =
                        "warehouse da guild indisponível: Guild ainda não carregada".into();
                    return;
                };
                let Some(character_id) = self.network_character_id else {
                    self.network_status = "warehouse da guild requer conexão online".into();
                    return;
                };
                if let Some(network) = &self.network {
                    self.shop = None;
                    self.warehouse = Some(Vec::new());
                    self.warehouse_npc_id = Some(guild_id);
                    self.guild_warehouse_open = true;
                    self.warehouse_page = 0;
                    self.selected_inventory_uid = None;
                    self.selected_warehouse_uid = None;
                    network.guild_warehouse_open(character_id, guild_id);
                    self.network_status = "online: abrindo warehouse da guild".into();
                }
            }
            shop::GuildPanelAction::Invite => {
                if let Some(target_id) = self.selected_player_target().map(|(id, _)| id) {
                    if let Some(network) = &self.network {
                        network.send_guild_action(INVITE_JOIN, target_id);
                        self.network_status =
                            format!("online: convite de guild enviado para {target_id}");
                    }
                } else {
                    self.network_status = "selecione um jogador próximo primeiro".into();
                }
            }
            shop::GuildPanelAction::Leave => {
                if let Some(network) = &self.network {
                    network.send_guild_action(LEAVE_GUILD, 0);
                    self.network_status = "online: saindo da guild".into();
                }
            }
            shop::GuildPanelAction::Kick => {
                let Some(target_id) = self.selected_target else {
                    self.network_status = "selecione um membro da guild primeiro".into();
                    return;
                };
                if let Some(network) = &self.network {
                    network.send_guild_action(KICK_MEMBER, target_id);
                    self.network_status = format!("online: expulsando membro {target_id}");
                }
            }
        }
    }

    fn handle_trade_panel_action(&mut self, action: shop::TradePanelAction) {
        const REQUEST: u32 = 1;
        const CLOSE: u32 = 2;
        const ADD_ITEM: u32 = 6;
        const SET_MONEY: u32 = 7;
        const REMOVE_ITEM: u32 = 11;
        const ACCEPT: u32 = 10;
        let Some(network) = &self.network else {
            self.network_status = "trade indisponível: servidor offline".into();
            return;
        };
        match action {
            shop::TradePanelAction::Request => {
                let Some((target_id, _)) = self.selected_player_target() else {
                    self.target_pick = Some(TargetPick::Trade);
                    self.network_status = "clique no jogador para negociar".into();
                    return;
                };
                network.send_trade(target_id, REQUEST, 0);
                self.network_status = format!("online: solicitação de trade para {target_id}");
            }
            shop::TradePanelAction::AcceptRequest => {
                let Some(target_id) = self.trade.pending_request else {
                    self.network_status = "nenhuma solicitação de trade pendente".into();
                    return;
                };
                network.send_trade(target_id, REQUEST, 0);
            }
            shop::TradePanelAction::AddItem => {
                let Some(item) = self
                    .inventory
                    .iter()
                    .find(|item| {
                        !self
                            .trade
                            .own_items
                            .iter()
                            .any(|offered| offered.uid == item.uid)
                    })
                    .copied()
                else {
                    self.network_status = "não há item livre para oferecer".into();
                    return;
                };
                network.send_trade(item.uid, ADD_ITEM, 0);
                self.trade.own_items.push(item);
                self.pending_trade_items.push(item.uid);
                self.trade.own_confirmed = false;
                self.network_status = format!("online: item {} oferecido", item.item_type);
            }
            shop::TradePanelAction::RemoveOwnItem(index) => {
                let Some(item) = self.trade.own_items.get(index).copied() else {
                    return;
                };
                network.send_trade(item.uid, REMOVE_ITEM, 0);
                self.trade
                    .own_items
                    .retain(|offered| offered.uid != item.uid);
                self.pending_trade_items.retain(|uid| *uid != item.uid);
                self.trade.own_confirmed = false;
            }
            shop::TradePanelAction::SetMoney => {
                if self.trade.own_silver == 0 && self.silver > 0 {
                    network.send_trade(1, SET_MONEY, 0);
                    self.trade.own_silver = 1;
                    self.trade.own_confirmed = false;
                    self.network_status = "online: 1 silver oferecido".into();
                }
            }
            shop::TradePanelAction::Confirm => {
                let Some(target_id) = self.trade.peer_id else {
                    return;
                };
                network.send_trade(target_id, ACCEPT, 1);
                self.trade.own_confirmed = true;
                self.network_status = "online: aguardando confirmação do outro jogador".into();
            }
            shop::TradePanelAction::Cancel => {
                let target_id = self
                    .trade
                    .peer_id
                    .or(self.trade.pending_request)
                    .unwrap_or_default();
                network.send_trade(target_id, CLOSE, 0);
                self.trade = network::TradeView::default();
                self.pending_trade_items.clear();
                self.quick_panel = None;
            }
        }
    }

    fn handle_player_shop_panel_action(&mut self, action: shop::PlayerShopPanelAction) {
        let Some(network) = &self.network else {
            self.network_status = "banca indisponível: servidor offline".into();
            return;
        };
        let Some(character_id) = self.network_character_id else {
            self.network_status = "banca indisponível: personagem ainda não carregado".into();
            return;
        };
        match action {
            shop::PlayerShopPanelAction::Open => {
                network.create_player_shop(character_id);
                self.player_shop_open = true;
                self.network_status = "online: abrindo banca neste mapa".into();
            }
            shop::PlayerShopPanelAction::Close => {
                network.close_player_shop(character_id);
                self.player_shop_open = false;
                self.player_shop_items
                    .retain(|listing| listing.owner_id != character_id);
                self.network_status = "online: fechando banca".into();
            }
            shop::PlayerShopPanelAction::QueryTarget => {
                let Some((target_id, target_name)) = self.selected_player_target() else {
                    self.network_status = "selecione um jogador próximo primeiro".into();
                    return;
                };
                self.player_shop_items
                    .retain(|listing| listing.owner_id == character_id);
                network.player_shop_query(character_id, target_id);
                self.network_status = format!("online: consultando a banca de {target_name}");
            }
            shop::PlayerShopPanelAction::AddSelectedItem => {
                let Some(price) = self
                    .player_shop_price
                    .parse::<u32>()
                    .ok()
                    .filter(|price| *price > 0)
                else {
                    self.network_status = "informe um preço maior que zero".into();
                    return;
                };
                let Some(uid) = self.selected_inventory_uid else {
                    self.network_status = "selecione um item do inventário primeiro".into();
                    return;
                };
                if self
                    .player_shop_items
                    .iter()
                    .any(|listing| listing.owner_id == character_id && listing.item.uid == uid)
                {
                    self.network_status = "este item já está anunciado".into();
                    return;
                }
                network.player_shop_add(character_id, uid, price);
                self.network_status = format!("online: anunciando item {uid} por {price} silver");
            }
            shop::PlayerShopPanelAction::Buy(index) => {
                let Some(listing) = self.player_shop_items.get(index).cloned() else {
                    return;
                };
                if listing.owner_id == character_id {
                    return;
                }
                network.player_shop_buy(character_id, listing.item.uid, listing.owner_id);
                self.network_status = format!("online: comprando item {}", listing.item.item_type);
            }
            shop::PlayerShopPanelAction::Remove(index) => {
                let Some(listing) = self.player_shop_items.get(index).cloned() else {
                    return;
                };
                if listing.owner_id != character_id {
                    return;
                }
                network.player_shop_remove(character_id, listing.item.uid);
                self.player_shop_items.retain(|candidate| {
                    candidate.owner_id != character_id || candidate.item.uid != listing.item.uid
                });
                self.network_status =
                    format!("online: removendo anúncio {}", listing.item.item_type);
            }
        }
    }

    fn move_cursor(&mut self) {
        if self.dead_since.is_some() {
            return;
        }
        self.follow_target = None;
        self.pending_attack = None;
        self.pending_skill_attack = None;
        self.selected_target = None;
        self.pending_pickup = None;
        self.shop = None;
        self.warehouse = None;
        self.warehouse_npc_id = None;
        self.guild_warehouse_open = false;
        self.selected_inventory_uid = None;
        self.selected_warehouse_uid = None;
        if let Some(renderer) = &self.renderer {
            let camera = renderer.camera(&self.map, &self.player);
            let target = self
                .map
                .unproject([self.cursor[0] + camera[0], self.cursor[1] + camera[1]])
                .map(|v| v.round() as i32);
            if self.modifiers.control_key() {
                self.pending_attack = None;
                self.player.stop();
                if self.player.moving() || !self.pending_network_steps.is_empty() {
                    self.queued_jump = Some(target);
                    return;
                }
                self.start_jump(target);
            } else if self.modifiers.shift_key() {
                self.face_towards(target);
            } else {
                self.player.go(&self.map, target);
            }
        }
    }

    fn handle_network_event(&mut self, event: NetworkEvent) {
        match event {
            NetworkEvent::Status(status) => self.network_status = status,
            NetworkEvent::Guild { action, data, name } => {
                let description = name.clone().unwrap_or_else(|| "sem nome".to_owned());
                if action == 2 && data != 0 && self.character.guild.is_empty() {
                    self.pending_guild_invite = Some(data);
                    self.quick_panel = Some(shop::QuickPanel::Guild);
                } else if action == 14 && data == 0 {
                    self.character.guild.clear();
                    self.character.guild_rank.clear();
                    self.guild_members.clear();
                    self.guild_id = None;
                    self.pending_guild_invite = None;
                    if self.guild_warehouse_open {
                        self.warehouse = None;
                        self.warehouse_npc_id = None;
                        self.guild_warehouse_open = false;
                    }
                } else if let Some(guild_name) = name {
                    if action == 14 {
                        self.character.guild = guild_name;
                        self.guild_id = Some(data);
                        self.pending_guild_invite = None;
                    }
                }
                self.network_status =
                    format!("online: guild action {action} data {data} ({description})");
            }
            NetworkEvent::GuildMember(member) => {
                if self.network_character_id == Some(member.character_id) {
                    self.character.guild_rank = match member.rank {
                        1000 => "Leader",
                        990 => "Deputy Leader",
                        _ => "Member",
                    }
                    .to_owned();
                }
                self.guild_members
                    .retain(|existing| existing.character_id != member.character_id);
                self.guild_members.push(member);
                self.guild_members
                    .sort_by_key(|existing| (existing.rank != 1000, existing.character_id));
            }
            NetworkEvent::Team { action, target_id } => {
                match action {
                    0 => self.team_leader_id = Some(target_id),
                    4 => self.pending_team_invite = Some(target_id),
                    1 => self.pending_team_join_request = Some(target_id),
                    6 => {
                        self.team_members.clear();
                        self.team_leader_id = None;
                        self.team_join_enabled = true;
                        self.pending_team_join_request = None;
                    }
                    8 => self.team_join_enabled = false,
                    9 => self.team_join_enabled = true,
                    15 => self.team_leader_id = Some(target_id),
                    _ => {}
                }
                self.network_status = format!("online: team action {action} target {target_id}");
            }
            NetworkEvent::TeamMember { member, action } => {
                if action == 0 {
                    self.team_members
                        .retain(|existing| existing.member_id != member.member_id);
                    self.team_members.push(member);
                    self.team_members.sort_by_key(|member| member.member_id);
                } else {
                    self.team_members
                        .retain(|existing| existing.member_id != member.member_id);
                    if self.team_members.is_empty() {
                        self.team_leader_id = None;
                    }
                }
            }
            NetworkEvent::Trade {
                target_id,
                subtype,
                data,
            } => match subtype {
                1 => {
                    self.trade.pending_request = Some(target_id);
                    self.trade.peer_name = self.actor_name(target_id);
                    self.trade.active = false;
                    self.quick_panel = Some(shop::QuickPanel::Trade);
                    self.network_status = format!("online: trade request from {target_id}");
                }
                3 => {
                    self.trade = network::TradeView {
                        active: true,
                        peer_id: Some(target_id),
                        peer_name: self.actor_name(target_id),
                        ..Default::default()
                    };
                    self.pending_trade_items.clear();
                    self.quick_panel = Some(shop::QuickPanel::Trade);
                    self.network_status = format!("online: trade opened with {target_id}");
                }
                5 | 17 => {
                    self.trade = network::TradeView::default();
                    self.pending_trade_items.clear();
                    if self.quick_panel == Some(shop::QuickPanel::Trade) {
                        self.quick_panel = None;
                    }
                    self.network_status = "online: trade closed".into();
                }
                8 => self.trade.peer_silver = data,
                10 => self.trade.peer_confirmed = data != 0,
                11 => self.trade.peer_items.retain(|item| item.uid != data),
                6 => {
                    self.pending_trade_items.retain(|uid| *uid != data);
                    if let Some(item) = self.inventory.iter().find(|item| item.uid == data).copied()
                        && !self
                            .trade
                            .own_items
                            .iter()
                            .any(|offered| offered.uid == data)
                    {
                        self.trade.own_items.push(item);
                        self.trade.own_items.sort_by_key(|item| item.position);
                    }
                }
                _ => {}
            },
            NetworkEvent::TradeItem(item) => {
                self.trade
                    .peer_items
                    .retain(|existing| existing.uid != item.uid);
                self.trade.peer_items.push(item);
                self.trade.peer_items.sort_by_key(|item| item.position);
            }
            NetworkEvent::VendorItem {
                owner_id,
                price,
                item,
            } => {
                self.player_shop_items
                    .retain(|listing| listing.owner_id != owner_id || listing.item.uid != item.uid);
                self.player_shop_items.push(network::PlayerShopItem {
                    owner_id,
                    price,
                    item,
                });
                self.player_shop_items.sort_by_key(|listing| listing.price);
                self.network_status =
                    format!("online: player shop listing received from {owner_id}");
            }
            NetworkEvent::Friend {
                friend_id,
                action,
                name,
                online,
            } => {
                let state = if online { "online" } else { "offline" };
                match action {
                    // Someone asked to be our friend: Accept/Refuse in the
                    // friends panel.
                    10 => {
                        self.pending_friend_request = Some((friend_id, name.clone()));
                        self.quick_panel = Some(shop::QuickPanel::Friends);
                        self.status_open = false;
                        self.inventory_open = false;
                        self.network_status = format!("{name} quer ser seu amigo");
                        return;
                    }
                    12 | 13 | 15 => {
                        self.friends.retain(|entry| entry.friend_id != friend_id);
                        self.friends.push(SocialEntry {
                            friend_id,
                            name: name.clone(),
                            online,
                        });
                        self.friends.sort_by_key(|entry| entry.name.to_lowercase());
                    }
                    14 => self.friends.retain(|entry| entry.friend_id != friend_id),
                    18 => self.ignores.retain(|entry| entry.friend_id != friend_id),
                    19 => {
                        self.ignores.retain(|entry| entry.friend_id != friend_id);
                        self.ignores.push(SocialEntry {
                            friend_id,
                            name: name.clone(),
                            online,
                        });
                        self.ignores.sort_by_key(|entry| entry.name.to_lowercase());
                    }
                    _ => {}
                }
                self.network_status =
                    format!("friend {name} ({friend_id}) action {action} {state}");
            }
            NetworkEvent::ChatMessage(message) => {
                let display = if message.channel == 2001 {
                    format!("Whisper from {}: {}", message.sender_name, message.message)
                } else {
                    format!("{}: {}", message.sender_name, message.message)
                };
                self.chat_history.push_back(message);
                while self.chat_history.len() > 40 {
                    self.chat_history.pop_front();
                }
                self.network_status = format!("online: {display}");
            }
            NetworkEvent::Skill {
                skill_id,
                level,
                experience,
            } => {
                self.action_bar.assign_skill(skill_id as u32);
                self.known_skills
                    .insert(skill_id as u32, (level, experience));
                self.network_status = format!("online: skill {skill_id} level {level}");
            }
            NetworkEvent::Stamina(stamina) => self.stamina = stamina,
            NetworkEvent::ActorLife { id, maximum, value } => {
                let entry = self.structure_lives.entry(id).or_insert((value, value));
                if maximum {
                    entry.1 = value;
                } else {
                    entry.0 = value;
                }
                let (life, max) = *entry;
                if let Some(actor) = self.remote_actors.get_mut(&id) {
                    actor.health = scaled_life(life, max);
                    actor.maximum_health = 1_000;
                }
            }
            NetworkEvent::SkillForgotten(skill_id) => {
                self.known_skills.remove(&skill_id);
                if self.active_skill == Some(skill_id) {
                    self.active_skill = None;
                }
            }
            NetworkEvent::WeaponSkill {
                weapon_type,
                level,
                experience,
            } => {
                self.weapon_skills.insert(weapon_type, (level, experience));
            }
            NetworkEvent::LevelUp { level } => {
                self.level = self.level.max(level);
                self.level_up_until = self.started.elapsed().as_secs_f32() + LEVEL_UP_DURATION;
            }
            NetworkEvent::Ready(ready) => {
                self.player.position = ready.position.map(|value| value as f32);
                self.character.name = ready.character_name;
                self.silver = ready.silver;
                self.cps = ready.cps;
                self.character.health = ready.health;
                self.character.maximum_health = ready.maximum_health;
                self.mana = ready.mana;
                self.maximum_mana = ready.maximum_mana;
                self.level = ready.level;
                self.initial_sync_complete = true;
                self.experience = ready.experience;
                self.character.strength = ready.strength;
                self.character.agility = ready.agility;
                self.character.vitality = ready.vitality;
                self.character.spirit = ready.spirit;
                self.character.attribute_points = ready.attribute_points;
                self.character.current_class = ready.current_class;
                self.character.previous_class = ready.previous_class;
                self.character.rebirths = ready.rebirths;
                self.character.kill_points = ready.kill_points;
                self.confirmed_network_cell = Some(ready.position);
                self.pending_network_steps.clear();
                self.player_shop_items.clear();
                self.network_status =
                    format!("online: char {} map {}", ready.character_id, ready.map_id);
            }
            NetworkEvent::Silver(silver) => self.silver = silver,
            NetworkEvent::Cps(cps) => self.cps = cps,
            NetworkEvent::Vitals {
                health,
                maximum_health,
                mana,
                maximum_mana,
                level,
                experience,
            } => {
                if self.initial_sync_complete && level > self.level {
                    self.level_up_until = self.started.elapsed().as_secs_f32() + LEVEL_UP_DURATION;
                }
                self.level = level;
                self.experience = experience;
                if self.initial_sync_complete && health == 0 {
                    self.on_own_death();
                } else if health > 0 && self.dead_since.is_some() {
                    self.on_own_revive();
                }
                self.character.health = health;
                self.character.maximum_health = maximum_health;
                self.mana = mana;
                self.maximum_mana = maximum_mana;
            }
            NetworkEvent::Attributes {
                strength,
                agility,
                vitality,
                spirit,
                attribute_points,
                current_class,
                previous_class,
                rebirths,
                kill_points,
                spouse,
            } => {
                self.character.kill_points = kill_points;
                self.character.spouse = spouse;
                self.character.strength = strength;
                self.character.agility = agility;
                self.character.vitality = vitality;
                self.character.spirit = spirit;
                self.character.attribute_points = attribute_points;
                self.character.current_class = current_class;
                self.character.previous_class = previous_class;
                self.character.rebirths = rebirths;
                if let Some(renderer) = &mut self.renderer {
                    renderer.invalidate_status();
                }
            }
            NetworkEvent::Nobility {
                character_id,
                donation,
                rank,
                remaining,
                title_id,
            } => {
                if self.network_character_id == Some(character_id) {
                    self.nobility_donation = donation;
                    self.nobility_rank = rank;
                    self.nobility_remaining = remaining;
                    self.equipped_title = title_id;
                    if let Some(renderer) = &mut self.renderer {
                        renderer.invalidate_status();
                    }
                }
            }
            NetworkEvent::NobilityRanking(entry) => {
                if entry.position == 1 {
                    self.nobility_ranking.clear();
                }
                self.nobility_ranking
                    .retain(|existing| existing.position != entry.position);
                self.nobility_ranking.push(entry);
                self.nobility_ranking.sort_by_key(|entry| entry.position);
                if let Some(renderer) = &mut self.renderer {
                    renderer.invalidate_status();
                }
            }
            NetworkEvent::ExperienceGain { experience } => {
                self.experience = experience;
            }
            NetworkEvent::InventoryItem(item) => {
                debug_log::write(format_args!(
                    "app received inventory item: uid={} type={} position={}",
                    item.uid, item.item_type, item.position
                ));
                if let Some(panel) = &mut self.item_action_panel
                    && panel.item.uid == item.uid
                {
                    panel.item = item;
                    if panel.action == shop::ItemContextAction::Rarity {
                        panel.selected_rarity = item.rarity.saturating_add(1).min(4);
                    }
                }
                let sort_acknowledged = self
                    .pending_inventory_sort_move
                    .is_some_and(|(uid, target)| uid == item.uid && target == item.position);
                // A move response can arrive as a pair (moved item followed
                // by the swapped item). Reconcile by UID only: removing an
                // unrelated item merely because it temporarily occupies the
                // same slot loses items when several moves are in flight.
                self.inventory.retain(|owned| owned.uid != item.uid);
                self.inventory.push(item);
                self.inventory.sort_by_key(|owned| owned.position);
                if self.pending_inventory_move.is_some_and(|(moved, _)| {
                    moved.uid == item.uid && moved.position != item.position
                }) {
                    self.pending_inventory_move = None;
                }
                if sort_acknowledged {
                    self.pending_inventory_sort_move = None;
                    self.advance_inventory_sort();
                }
            }
            NetworkEvent::EquipmentItem(item) => {
                if let Some(panel) = &mut self.item_action_panel
                    && panel.item.uid == item.uid
                {
                    panel.item = item;
                    if panel.action == shop::ItemContextAction::Rarity {
                        panel.selected_rarity = item.rarity.saturating_add(1).min(4);
                    }
                }
                self.equipment
                    .retain(|owned| owned.position != item.position);
                if let Some(hidden) = self
                    .hidden_default_equipment_slots
                    .get_mut(usize::from(item.position))
                {
                    *hidden = false;
                }
                self.equipment.push(item);
                self.equipment.sort_by_key(|owned| owned.position);
                self.refresh_character_equipment_model();
            }
            NetworkEvent::EquipmentItemRemoved { uid, slot } => {
                debug_log::write(format_args!(
                    "app removed equipment item: uid={} slot={}",
                    uid, slot
                ));
                self.equipment
                    .retain(|item| item.uid != uid && (slot > 9 || item.position != slot));
                self.hide_default_equipment_slot(slot);
                self.refresh_character_equipment_model();
            }
            NetworkEvent::InventoryItemRemoved(uid) => {
                debug_log::write(format_args!("app received inventory removal: uid={uid}"));
                self.inventory.retain(|item| item.uid != uid);
                if self.pending_inventory_sort.is_some()
                    || self.pending_inventory_sort_move.is_some()
                {
                    self.pending_inventory_sort = None;
                    self.pending_inventory_sort_move = None;
                    self.network_status =
                        "sort cancelado: o servidor removeu um item; inventário será sincronizado"
                            .into();
                    if let (Some(network), Some(character_id)) =
                        (&self.network, self.network_character_id)
                    {
                        network.request_items(character_id);
                    }
                }
                if self
                    .pending_inventory_drop
                    .is_some_and(|item| item.uid == uid)
                {
                    self.pending_inventory_drop = None;
                }
                if self.selected_inventory_uid == Some(uid) {
                    self.selected_inventory_uid = None;
                }
            }
            NetworkEvent::WarehouseOpened(npc_id) => {
                self.shop = None;
                self.warehouse_npc_id = Some(npc_id);
                self.guild_warehouse_open = false;
                self.warehouse = Some(Vec::new());
                self.warehouse_page = 0;
                self.selected_inventory_uid = None;
                self.selected_warehouse_uid = None;
                self.warehouse_amount = "1".into();
                self.warehouse_amount_focused = false;
                self.dialog = None;
            }
            NetworkEvent::WarehouseBalance(silver) => {
                self.warehouse_silver = silver;
                self.warehouse_amount = "1".into();
                self.warehouse_amount_focused = false;
            }
            NetworkEvent::WarehouseItem(item) => {
                let warehouse = self.warehouse.get_or_insert_with(Vec::new);
                warehouse.retain(|owned| owned.position != item.position);
                warehouse.push(item);
                warehouse.sort_by_key(|owned| owned.position);
            }
            NetworkEvent::WarehouseItemRemoved(uid) => {
                if let Some(warehouse) = &mut self.warehouse {
                    warehouse.retain(|item| item.uid != uid);
                }
                if self.selected_warehouse_uid == Some(uid) {
                    self.selected_warehouse_uid = None;
                }
            }
            NetworkEvent::GuildWarehouseOpened(guild_id) => {
                self.shop = None;
                self.warehouse_npc_id = Some(guild_id);
                self.guild_warehouse_open = true;
                self.warehouse = Some(Vec::new());
                self.warehouse_page = 0;
                self.selected_inventory_uid = None;
                self.selected_warehouse_uid = None;
                self.dialog = None;
            }
            NetworkEvent::GuildWarehouseItem(item) => {
                let warehouse = self.warehouse.get_or_insert_with(Vec::new);
                warehouse.retain(|owned| owned.position != item.position);
                warehouse.push(item);
                warehouse.sort_by_key(|owned| owned.position);
            }
            NetworkEvent::GuildWarehouseItemRemoved(uid) => {
                if let Some(warehouse) = &mut self.warehouse {
                    warehouse.retain(|item| item.uid != uid);
                }
                if self.selected_warehouse_uid == Some(uid) {
                    self.selected_warehouse_uid = None;
                }
            }
            NetworkEvent::Notice(message) => {
                debug_log::write(format_args!("app received notice: {message}"));
                if message.contains("Rarity")
                    || message.contains("rarity")
                    || message.contains("reroll")
                {
                    self.network_status = message.clone();
                }
                if message.contains("Player shop opened") {
                    self.player_shop_open = true;
                } else if message.contains("Player shop closed")
                    || message.contains("Player shops are not available")
                {
                    self.player_shop_open = false;
                }
                let trade_rejected = [
                    "Only inventory items can be traded.",
                    "You are not in a trade.",
                    "That trade request is no longer valid.",
                    "The other inventory cannot hold this trade.",
                    "That item is already in the trade.",
                    "That item is not in the trade.",
                    "That trade amount is invalid.",
                ]
                .iter()
                .any(|error| message.contains(error));
                if trade_rejected {
                    for uid in self.pending_trade_items.drain(..).collect::<Vec<_>>() {
                        self.trade.own_items.retain(|item| item.uid != uid);
                    }
                    self.trade.own_confirmed = false;
                }
                let legacy_item_action_rejected = message.contains("Missing Item Action Type");
                if message.contains("Inventory move rejected") || legacy_item_action_rejected {
                    if let Some((moved, swapped)) = self.pending_inventory_move.take() {
                        self.restore_inventory_move(moved, swapped);
                    }
                    if self.pending_inventory_sort.is_some()
                        || self.pending_inventory_sort_move.is_some()
                    {
                        self.pending_inventory_sort = None;
                        self.pending_inventory_sort_move = None;
                        self.network_status =
                            "sort interrompido: sincronizando inventário com o servidor".into();
                        if let (Some(network), Some(character_id)) =
                            (&self.network, self.network_character_id)
                        {
                            network.request_items(character_id);
                        }
                    }
                }
                if message.contains("Item drop rejected") || legacy_item_action_rejected {
                    if let Some(item) = self.pending_inventory_drop.take() {
                        self.inventory.push(item);
                        self.inventory.sort_by_key(|owned| owned.position);
                    }
                }
                self.network_status = format!("online: {message}");
            }
            NetworkEvent::WalkConfirmed {
                character_id,
                direction,
            } => {
                if let Some(position) = self.pending_network_steps.pop_front() {
                    self.confirmed_network_cell = Some(position);
                }
                self.network_status =
                    format!("online: walk confirmed {character_id} dir {direction}");
            }
            NetworkEvent::JumpConfirmed {
                character_id,
                position,
                direction,
            } => {
                self.pending_network_jump = false;
                self.pending_network_steps.clear();
                self.confirmed_network_cell = Some(position);
                self.network_status = format!(
                    "online: jump confirmed {character_id} to ({}, {}) dir {direction}",
                    position[0], position[1]
                );
            }
            NetworkEvent::MovementRejected(message) => {
                self.pending_network_jump = false;
                self.pending_network_steps.clear();
                if let Some(position) = self.confirmed_network_cell {
                    self.player.reposition(position);
                }
                self.network_status = format!("online: movement rejected ({message})");
            }
            NetworkEvent::Reposition {
                map_id,
                position,
                direction,
            } => {
                self.pending_network_jump = false;
                self.shop = None;
                self.warehouse = None;
                self.warehouse_npc_id = None;
                self.guild_warehouse_open = false;
                self.selected_inventory_uid = None;
                self.selected_warehouse_uid = None;
                self.pending_network_steps.clear();
                self.confirmed_network_cell = Some(position);
                self.portal_armed = false;
                if self.map.id != map_id {
                    match map::Map::load(&mut self.assets, map_id) {
                        Ok(map) => {
                            self.map = map;
                            self.remote_actors.clear();
                            self.remote_models.clear();
                            self.remote_npcs.clear();
                        }
                        Err(error) => {
                            self.network_status =
                                format!("online: map {map_id} unavailable after teleport: {error}");
                        }
                    }
                }
                self.player.reposition(position);
                self.network_status = format!("online: repositioned map {map_id} dir {direction}");
            }
            NetworkEvent::RemoteWalk {
                id,
                direction,
                movement_type,
            } => {
                if let Some(actor) = self.remote_actors.get_mut(&id) {
                    let delta = walk_delta(direction);
                    let now = self.started.elapsed().as_secs_f32();
                    // If another packet arrives while moving, continue from the
                    // current visual position so the actor never snaps backward.
                    if actor.movement_until > actor.movement_started {
                        let t = ((now - actor.movement_started)
                            / (actor.movement_until - actor.movement_started))
                            .clamp(0.0, 1.0);
                        actor.position[0] = actor.movement_from[0]
                            + (actor.movement_to[0] - actor.movement_from[0]) * t;
                        actor.position[1] = actor.movement_from[1]
                            + (actor.movement_to[1] - actor.movement_from[1]) * t;
                    }
                    actor.movement_from = actor.position;
                    actor.movement_to = [
                        actor.position[0] + delta[0] as f32,
                        actor.position[1] + delta[1] as f32,
                    ];
                    actor.movement_started = now;
                    actor.movement_until = now
                        + if movement_type == 1 {
                            1.0 / 6.0
                        } else {
                            1.0 / 3.0
                        };
                    actor.direction = direction;
                    actor.action = if movement_type == 1 {
                        if actor.step_right { 121 } else { 120 }
                    } else if actor.step_right {
                        111
                    } else {
                        110
                    };
                    actor.step_right = !actor.step_right;
                    actor.animation_until = self.started.elapsed().as_secs_f32()
                        + if movement_type == 1 {
                            1.0 / 6.0
                        } else {
                            1.0 / 3.0
                        };
                    self.network_status =
                        format!("online: actor {id} moved dir {direction} type {movement_type}");
                }
            }
            NetworkEvent::KillMode(mode) => {
                if usize::from(mode) < shop::KILL_MODES.len() {
                    self.kill_mode = mode;
                }
            }
            NetworkEvent::RemoteFacing { id, direction } => {
                if let Some(actor) = self.remote_actors.get_mut(&id) {
                    actor.direction = direction;
                }
            }
            NetworkEvent::RemoteJump {
                id,
                position,
                direction,
            } => {
                if let Some(actor) = self.remote_actors.get_mut(&id) {
                    // Fly from where the actor is drawn now to the landing
                    // cell over the same time the local player's jump takes;
                    // the renderer lifts it along the arc while action is 130.
                    let now = self.started.elapsed().as_secs_f32();
                    if actor.movement_until > actor.movement_started && now < actor.movement_until {
                        let t = ((now - actor.movement_started)
                            / (actor.movement_until - actor.movement_started))
                            .clamp(0.0, 1.0);
                        actor.position[0] = actor.movement_from[0]
                            + (actor.movement_to[0] - actor.movement_from[0]) * t;
                        actor.position[1] = actor.movement_from[1]
                            + (actor.movement_to[1] - actor.movement_from[1]) * t;
                    }
                    let target = position.map(|value| value as f32);
                    let motion = motion_math::jump_motion(
                        actor.position,
                        target,
                        self.character.duration(Action::Jump),
                    );
                    // Face the way the jump goes; the packet's direction is
                    // only a fallback for a jump in place (older servers
                    // forwarded 0 there).
                    actor.direction = if target == actor.position {
                        direction
                    } else {
                        direction_towards(actor.position, target)
                    };
                    actor.movement_from = actor.position;
                    actor.movement_to = target;
                    actor.movement_started = now;
                    actor.movement_until = now + motion.duration.as_secs_f32();
                    actor.action = 130;
                    actor.animation_started = now;
                    actor.animation_until = actor.movement_until;
                }
            }
            NetworkEvent::RemoteAction { id, action } => {
                if let Some(actor) = self.remote_actors.get_mut(&id) {
                    let now = self.started.elapsed().as_secs_f32();
                    if action == 131 {
                        actor.respawn_until = now + 0.9;
                    }
                    actor.action = action as u8;
                    actor.animation_started = now;
                    actor.animation_until = now + action_duration(action, &self.character);
                }
            }
            NetworkEvent::RemoteCombat {
                id,
                target_id,
                action,
                value,
            } => {
                if action == 14 {
                    if self.network_character_id == Some(target_id) {
                        self.on_own_death();
                    }
                    if self.network_character_id == Some(id) {
                        self.kill_count = self.kill_count.saturating_add(1);
                    }
                    let now = self.started.elapsed().as_secs_f32();
                    let fall = self.character.duration(Action::Die).as_secs_f32().max(0.1);
                    if let Some(actor) = self.remote_actors.get_mut(&target_id) {
                        actor.health = 0;
                        let player = !actor.npc && !(400001..=499999).contains(&target_id);
                        // Players stay on the ground until they revive;
                        // monsters fade after their death animation.
                        if player {
                            actor.action = renderer::REMOTE_DEAD;
                            actor.animation_started = now;
                            actor.animation_until = now + fall;
                        } else {
                            actor.action = 160;
                            actor.animation_until = now + 1.2;
                        }
                    }
                    if self.pending_attack == Some(target_id) {
                        self.pending_attack = None;
                    }
                    if self
                        .pending_skill_attack
                        .is_some_and(|(target, _)| target == target_id)
                    {
                        self.pending_skill_attack = None;
                    }
                }
                if let Some(actor) = self.remote_actors.get_mut(&id) {
                    // Interaction packets carry the wire combat action (2),
                    // while the renderer uses the player's motion catalog.
                    actor.action = 140;
                    actor.animation_until = self.started.elapsed().as_secs_f32() + 0.45;
                }
                self.network_status =
                    format!("online: combat {id} -> {target_id}, hit {value} (action {action})");
                let now = self.started.elapsed().as_secs_f32();
                // A physical blow of 0 missed: it shows "Miss".
                if value > 0 || action == 2 {
                    self.combat_numbers.push((target_id, value, now + 1.0));
                }
                if self.network_character_id == Some(target_id) {
                    self.hit_until = now + 0.45;
                    self.player.action = Action::Hit;
                    self.player.action_instance = self.player.action_instance.wrapping_add(1);
                    self.local_action = Some((Action::Hit, now, now + 0.45));
                } else if self.network_character_id == Some(id) {
                    self.player.action = Action::Attack;
                    self.player.action_instance = self.player.action_instance.wrapping_add(1);
                    self.local_action = Some((Action::Attack, now, now + 0.45));
                }
            }
            NetworkEvent::SkillEffect {
                attacker_id,
                target_id,
                skill_id,
                damage,
            } => {
                let now = self.started.elapsed().as_secs_f32();
                let level = self
                    .known_skills
                    .get(&u32::from(skill_id))
                    .filter(|_| self.network_character_id == Some(attacker_id))
                    .map_or(0, |(level, _)| *level);
                self.skill_effects.push(renderer::SkillEffectInstance {
                    attacker_id,
                    target_id,
                    skill_id: u32::from(skill_id),
                    level,
                    started: now,
                    // The renderer stops each effect when its animation ends.
                    expires: now + renderer::SKILL_EFFECT_MAX_SECONDS,
                });
                if damage > 0 {
                    self.combat_numbers.push((target_id, damage, now + 1.0));
                }
                if let Some(actor) = self.remote_actors.get_mut(&attacker_id) {
                    actor.action = 140;
                    actor.animation_started = now;
                    actor.animation_until = now + 0.45;
                }
                if self.network_character_id == Some(target_id) {
                    self.hit_until = now + 0.45;
                    self.player.action = Action::Hit;
                    self.player.action_instance = self.player.action_instance.wrapping_add(1);
                    self.local_action = Some((Action::Hit, now, now + 0.45));
                } else if self.network_character_id == Some(attacker_id) {
                    self.player.action = Action::Attack;
                    self.player.action_instance = self.player.action_instance.wrapping_add(1);
                    self.local_action = Some((Action::Attack, now, now + 0.45));
                }
                self.network_status =
                    format!("online: skill {skill_id} {attacker_id} -> {target_id}, hit {damage}");
            }
            NetworkEvent::GroundItem {
                uid,
                item_id,
                position,
                drop_type,
            } => {
                if drop_type == 2 {
                    self.ground_loot
                        .retain(|(existing, _, _, _)| *existing != uid);
                    if self.pending_pickup.is_some_and(|pending| pending.0 == uid) {
                        self.pending_pickup = None;
                    }
                } else {
                    self.ground_loot
                        .retain(|(existing, old_position, old_item, _)| {
                            *existing != uid && (*old_item != item_id || *old_position != position)
                        });
                    self.ground_loot.push((
                        uid,
                        position,
                        item_id,
                        self.started.elapsed().as_secs_f32() + 50.0,
                    ));
                }
            }
            NetworkEvent::NpcDialog(dialog) => {
                if self.shop.as_ref().is_some_and(|shop| {
                    dialog.task_id == shop.npc_id
                        || (dialog.action == 100 && dialog.messages.is_empty())
                }) {
                    return;
                }
                let opens_shop = dialog
                    .messages
                    .iter()
                    .any(|message| message.contains("catalog items"));
                let state = self.dialog.get_or_insert_with(|| DialogState {
                    task_id: dialog.task_id,
                    avatar: 0,
                    lines: Vec::new(),
                    options: Vec::new(),
                    edit: None,
                    input: String::new(),
                    input_focused: false,
                });
                if dialog.task_id != 0 {
                    state.task_id = dialog.task_id;
                }
                if dialog.action == 4 {
                    state.avatar = dialog.avatar;
                }
                for message in dialog.messages {
                    if dialog.action == 3 {
                        state.edit = Some((dialog.option_id, message.clone()));
                        state.input_focused = true;
                        state.options.push((dialog.option_id, message));
                        state.refresh_edit_row();
                    } else if dialog.action == 2 {
                        state.options.push((dialog.option_id, message));
                    } else if !message.is_empty() {
                        state.lines.push(message);
                    }
                }
                if opens_shop {
                    let shop_npc_id = state.task_id;
                    match shop::ShopState::load(&mut self.assets, shop_npc_id) {
                        Ok(Some(shop)) => {
                            self.warehouse = None;
                            self.warehouse_npc_id = None;
                            self.guild_warehouse_open = false;
                            self.selected_inventory_uid = None;
                            self.selected_warehouse_uid = None;
                            self.shop = Some(shop);
                            self.dialog = None;
                            self.network_status = "online: shop opened".into();
                        }
                        Ok(None) => {
                            self.network_status =
                                format!("online: shop {shop_npc_id} has no local catalog");
                        }
                        Err(error) => {
                            self.network_status = format!("online: shop unavailable: {error}");
                        }
                    }
                }
                if !opens_shop {
                    self.network_status = "online: NPC dialog received".into();
                }
            }
            NetworkEvent::RemoveActor(id) => {
                self.remote_actors.remove(&id);
                self.remote_models.remove(&id);
                self.remote_npcs.remove(&id);
                self.remote_monsters.remove(&id);
                self.remote_models_loading.remove(&id);
                self.remote_npcs_loading.remove(&id);
                self.remote_monsters_loading.remove(&id);
                self.network_status = format!(
                    "online: actor {id} left screen ({} actors)",
                    self.remote_actors.len()
                );
            }
            NetworkEvent::PlayerSpawn(mut actor) | NetworkEvent::NpcSpawn(mut actor) => {
                if let Some(&(life, max)) = self.structure_lives.get(&actor.id) {
                    actor.health = scaled_life(life, max);
                    actor.maximum_health = 1_000;
                }
                let monster = !actor.npc && (400001..=499999).contains(&actor.id);
                if monster && actor.health == 0 {
                    if let Some(existing) = self.remote_actors.get_mut(&actor.id) {
                        existing.health = 0;
                        existing.action = 160;
                        existing.animation_until = self.started.elapsed().as_secs_f32() + 1.2;
                    }
                    return;
                }
                let kind = if actor.npc {
                    "npc"
                } else if monster {
                    "monster"
                } else {
                    "player"
                };
                let id = actor.id;
                let respawn_until = self.started.elapsed().as_secs_f32() + 0.9;
                if !actor.npc && self.network_character_id == Some(id) {
                    self.status_flags = actor.status_flags;
                    self.renderer
                        .as_mut()
                        .map(|renderer| renderer.invalidate_hud());
                }
                if !actor.npc && self.network_character_id == Some(id) && actor.body != 0 {
                    let armor = self.appearance_armor(actor.armor);
                    let helmet = actor.helmet;
                    let right_weapon = actor.right_weapon;
                    let left_weapon = actor.left_weapon;
                    debug_log::write(format_args!(
                        "local world appearance update: helmet={} armor={} right_weapon={} left_weapon={}",
                        helmet, armor, right_weapon, left_weapon
                    ));
                    let job = RemoteModelJob::Player {
                        id,
                        body: actor.body,
                        armor,
                        helmet,
                        hair: actor.hair_style,
                        right_weapon,
                        left_weapon,
                        name: actor.name.clone(),
                        health: actor
                            .health
                            .max(self.character.health.min(u16::MAX as u32) as u16),
                    };
                    if self
                        .remote_model_loader
                        .as_ref()
                        .is_none_or(|loader| loader.requests.send(job).is_err())
                    {
                        eprintln!("Local character model update could not be queued");
                    }
                    // The game server broadcasts MsgPlayer updates to the
                    // actor's screen as well as sending them directly to the
                    // actor. This packet is an appearance refresh for the
                    // local paper doll, not a second world actor.
                    self.remote_actors.remove(&id);
                    self.remote_models.remove(&id);
                    return;
                }
                if !actor.npc && self.network_character_id == Some(id) {
                    self.remote_actors.remove(&id);
                    self.remote_models.remove(&id);
                    return;
                }
                // The server re-sends MsgPlayer when a player changes gear;
                // rebuild the model then. The old one stays drawn until the
                // new one loads, and results apply in request order.
                let appearance_changed = !actor.npc
                    && self.remote_actors.get(&id).is_some_and(|old| {
                        (old.body, old.armor, old.helmet, old.hair_style, old.right_weapon, old.left_weapon)
                            != (
                                actor.body,
                                actor.armor,
                                actor.helmet,
                                actor.hair_style,
                                actor.right_weapon,
                                actor.left_weapon,
                            )
                    });
                if !actor.npc
                    && actor.body != 0
                    && (appearance_changed
                        || (!self.remote_models.contains_key(&id)
                            && !self.remote_models_loading.contains(&id)))
                {
                    self.remote_models_loading.insert(id);
                    let job = RemoteModelJob::Player {
                        id,
                        body: actor.body,
                        armor: actor.armor,
                        helmet: actor.helmet,
                        hair: actor.hair_style,
                        right_weapon: actor.right_weapon,
                        left_weapon: actor.left_weapon,
                        name: actor.name.clone(),
                        health: actor.health,
                    };
                    if self
                        .remote_model_loader
                        .as_ref()
                        .is_none_or(|loader| loader.requests.send(job).is_err())
                    {
                        self.remote_models_loading.remove(&id);
                    }
                }
                // A gate opening or closing comes back with another look.
                if actor.npc
                    && self
                        .remote_actors
                        .get(&id)
                        .is_some_and(|old| old.mesh != actor.mesh)
                {
                    self.remote_npcs.remove(&id);
                }
                if actor.npc
                    && !self.remote_npcs.contains_key(&id)
                    && self.remote_npcs_loading.insert(id)
                {
                    let job = RemoteModelJob::Npc {
                        id,
                        kind: actor.npc_kind,
                        mesh: actor.mesh as u16,
                    };
                    if self
                        .remote_model_loader
                        .as_ref()
                        .is_none_or(|loader| loader.requests.send(job).is_err())
                    {
                        self.remote_npcs_loading.remove(&id);
                    }
                }
                if monster
                    && !self.remote_monsters.contains_key(&id)
                    && self.remote_monsters_loading.insert(id)
                {
                    let job = RemoteModelJob::Monster {
                        id,
                        mesh: actor.mesh,
                    };
                    if self
                        .remote_model_loader
                        .as_ref()
                        .is_none_or(|loader| loader.requests.send(job).is_err())
                    {
                        self.remote_monsters_loading.remove(&id);
                    }
                }
                actor.maximum_health = self
                    .remote_actors
                    .get(&id)
                    .map(|old| old.maximum_health.max(actor.health))
                    .unwrap_or(actor.health);
                if monster {
                    actor.respawn_until = respawn_until;
                }
                self.remote_actors.insert(id, actor);
                if let Some(actor) = self.remote_actors.get(&id) {
                    self.network_status = format!(
                        "online: {} {} mesh {} ({} actors)",
                        kind,
                        actor.name,
                        actor.mesh,
                        self.remote_actors.len()
                    );
                }
            }
            NetworkEvent::Error(error) => {
                debug_log::write(format_args!("app received network error: {error}"));
                self.pending_inventory_sort = None;
                self.pending_inventory_sort_move = None;
                if let Some((moved, swapped)) = self.pending_inventory_move.take() {
                    self.restore_inventory_move(moved, swapped);
                }
                if let Some(item) = self.pending_inventory_drop.take() {
                    self.inventory.push(item);
                    self.inventory.sort_by_key(|owned| owned.position);
                }
                self.network_status = format!("network error: {error}");
            }
        }
        if let Some(window) = &self.window {
            window.set_title(&format!("gravak-co — {}", self.network_status));
        }
    }
    fn send_network_step(&mut self, previous_cell: [i32; 2]) {
        if self.player.action == Action::Jump || self.pending_network_jump {
            return;
        }
        let Some(network) = &self.network else {
            return;
        };
        let Some(character_id) = self.network_character_id else {
            return;
        };
        let current_cell = self.player.cell();
        let previous_predicted = self
            .pending_network_steps
            .back()
            .copied()
            .or(self.confirmed_network_cell)
            .unwrap_or(previous_cell);
        if current_cell == previous_predicted {
            return;
        }
        let direction = direction_towards(
            previous_predicted.map(|value| value as f32),
            current_cell.map(|value| value as f32),
        );
        network.send_walk(character_id, direction);
        self.pending_network_steps.push_back(current_cell);
        self.network_status = format!("online: sending walk dir {direction}");
    }

    /// Turns the standing hero towards `target` and tells the server, which
    /// turns it on the other players' screens too.
    fn face_towards(&mut self, target: [i32; 2]) {
        let facing = self.player.facing;
        self.player.face(target);
        if self.player.facing == facing {
            return;
        }
        let current = self.confirmed_network_cell.unwrap_or(self.player.cell());
        if let (Some(network), Some(character_id)) = (&self.network, self.network_character_id) {
            let direction = direction_towards(
                current.map(|value| value as f32),
                target.map(|value| value as f32),
            );
            network.send_facing(character_id, direction, current);
        }
    }

    fn start_jump(&mut self, target: [i32; 2]) {
        let current = self.player.cell();
        let Some(target) =
            self.player
                .jump(&self.map, target, self.character.duration(Action::Jump))
        else {
            return;
        };
        let current = self.confirmed_network_cell.unwrap_or(current);
        if let (Some(network), Some(character_id)) = (&self.network, self.network_character_id) {
            self.pending_network_jump = true;
            network.send_jump(character_id, target, current);
            self.network_status = format!("online: sending jump to ({}, {})", target[0], target[1]);
        }
    }

    /// The actor drawn under the cursor, nearest the camera first:
    /// (id, npc, npc kind, name).
    fn actor_at_cursor(&self) -> Option<(u32, bool, u16, String)> {
        let renderer = self.renderer.as_ref()?;
        let camera = renderer.camera(&self.map, &self.player);
        self.remote_actors
            .values()
            .filter_map(|actor| {
                let projected = self.map.project(actor.position);
                let origin = [projected[0] - camera[0], projected[1] - camera[1]];
                let height = if actor.npc { 96.0 } else { 160.0 };
                let within = (self.cursor[0] - origin[0]).abs() <= 48.0
                    && self.cursor[1] >= origin[1] - height
                    && self.cursor[1] <= origin[1] + 24.0;
                within.then_some((actor.position[0] + actor.position[1], actor))
            })
            .min_by(|(left, _), (right, _)| left.total_cmp(right))
            .map(|(_, actor)| (actor.id, actor.npc, actor.npc_kind, actor.name.clone()))
    }

    /// Another player under the cursor in the world (not over the HUD or a
    /// menu), for the hover card.
    fn hovered_player(&self) -> Option<u32> {
        let size = self.window.as_ref()?.inner_size();
        let viewport = [size.width as f32, size.height as f32];
        if hud::contains(viewport, self.cursor)
            || self.choice_menu().is_some_and(|menu| menu.contains(self.cursor))
            || self.team_prompt().is_some_and(|prompt| prompt.contains(self.cursor))
        {
            return None;
        }
        let (id, npc, _, name) = self.actor_at_cursor()?;
        (!npc && !(400001..=499999).contains(&id) && !name.is_empty()).then_some(id)
    }

    fn select_actor_at_cursor(&mut self) -> bool {
        if self.renderer.is_none() {
            return false;
        }
        let selected = self.actor_at_cursor();
        if let Some((id, false, _, name)) = &selected
            && self.modifiers.shift_key()
            && !(400001..=499999).contains(id)
            && !name.is_empty()
        {
            let (id, name) = (*id, name.clone());
            self.open_player_menu(id, name);
            return true;
        }
        let Some((id, npc, npc_kind, name)) = selected else {
            // Clicking the ground cancels a pending target pick.
            if self.target_pick.take().is_some() {
                self.network_status = "seleção de alvo cancelada".into();
            }
            return false;
        };
        // Training ground Stakes and Scarecrows are attacked, not talked to;
        // the Guild War pole and gates with Ctrl held (StrRes 10352).
        let training_goal = npc
            && (is_training_goal(npc_kind)
                || (is_war_structure(npc_kind) && self.modifiers.control_key()));
        let npc = npc && !training_goal;
        self.held = false;
        if npc {
            self.dialog = None;
            self.shop = None;
        }
        let monster = training_goal || (!npc && (400001..=499999).contains(&id));
        let kind = if npc {
            "npc"
        } else if monster {
            "monster"
        } else {
            "player"
        };
        let label = if name.is_empty() { "unnamed" } else { &name };
        if npc && let Some(network) = &self.network {
            network.send_npc_interact(id, npc_kind);
        }
        if monster {
            self.pending_pickup = None;
            self.pending_skill_attack = None;
            self.selected_target = Some(id);
            self.pending_attack = Some(id);
            self.next_attack_at = 0.0;
            self.process_pending_attack();
            self.network_status = format!("online: approaching {label} ({id})");
        } else if !npc {
            self.selected_target = Some(id);
        }
        self.network_status = format!("online: selected {kind} {label} ({id})");
        if !npc && !monster && let Some(pick) = self.target_pick.take() {
            match pick {
                TargetPick::Friend => {
                    self.handle_friends_panel_action(shop::FriendsPanelAction::AddFriend);
                }
                TargetPick::Ignore => {
                    self.handle_friends_panel_action(shop::FriendsPanelAction::AddIgnore);
                }
                TargetPick::Trade => self.handle_trade_panel_action(shop::TradePanelAction::Request),
            }
        }
        if let Some(window) = &self.window {
            window.set_title(&format!("gravak-co — {}", self.network_status));
        }
        true
    }

    fn pickup_ground_loot_at_cursor(&mut self) -> bool {
        let Some((uid, item_id, x, y)) = self.ground_loot_at_cursor() else {
            return false;
        };
        if self.network.is_none() {
            return false;
        }
        self.pending_attack = None;
        self.pending_pickup = Some((uid, item_id, [i32::from(x), i32::from(y)]));
        self.held = false;
        self.network_status = format!("online: approaching loot {item_id}");
        self.process_pending_pickup();
        true
    }

    fn process_pending_pickup(&mut self) {
        let Some((uid, item_id, target)) = self.pending_pickup else {
            return;
        };
        if !self
            .ground_loot
            .iter()
            .any(|(existing, _, _, _)| *existing == uid)
        {
            self.pending_pickup = None;
            return;
        }
        let target_position = target.map(|value| value as f32);
        let distance = motion_math::cell_distance(self.player.position, target_position);
        if distance > 1.5 {
            if self.player.moving() || !self.pending_network_steps.is_empty() {
                return;
            }
            self.player.go(&self.map, target);
            self.network_status = format!("online: approaching loot {item_id}");
            return;
        }
        if self.player.moving() || !self.pending_network_steps.is_empty() {
            return;
        }
        let (Some(network), Some(character_id)) = (&self.network, self.network_character_id) else {
            return;
        };
        // The classic client starts the kneel locally and sends only the
        // ground-item request. The server removes the item and broadcasts
        // the 1101 result; it does not drive the pickup animation through
        // ChangeAction.
        network.send_pickup_ground_item(uid, item_id, target[0] as u16, target[1] as u16);
        self.face_towards(target);
        self.player.action = Action::Pickup;
        self.player.action_instance = self.player.action_instance.wrapping_add(1);
        let now = self.started.elapsed().as_secs_f32();
        self.local_action = Some((
            Action::Pickup,
            now,
            now + self.character.duration(Action::Pickup).as_secs_f32(),
        ));
        self.pending_pickup = None;
        self.network_status = format!("online: picking up loot {item_id} ({character_id})");
    }

    fn ground_loot_at_cursor(&self) -> Option<(u32, u32, u16, u16)> {
        let renderer = self.renderer.as_ref()?;
        let camera = renderer.camera(&self.map, &self.player);
        self.ground_loot
            .iter()
            .filter_map(|(uid, position, item_id, _)| {
                let projected = self.map.project(*position);
                let origin = [projected[0] - camera[0], projected[1] - camera[1]];
                let within = (self.cursor[0] - origin[0]).abs() <= 32.0
                    && self.cursor[1] >= origin[1] - 28.0
                    && self.cursor[1] <= origin[1] + 30.0;
                within.then_some((
                    (self.cursor[0] - origin[0]).powi(2) + (self.cursor[1] - origin[1]).powi(2),
                    *uid,
                    *item_id,
                    position[0].round().clamp(0.0, f32::from(u16::MAX)) as u16,
                    position[1].round().clamp(0.0, f32::from(u16::MAX)) as u16,
                ))
            })
            .min_by(|left, right| left.0.total_cmp(&right.0))
            .map(|(_, uid, item_id, x, y)| (uid, item_id, x, y))
    }

    fn process_pending_attack(&mut self) {
        let Some(target_id) = self.pending_attack else {
            return;
        };
        let Some(target) = self.remote_actors.get(&target_id) else {
            self.pending_attack = None;
            return;
        };
        let target_cell = target.position.map(|value| value.round() as i32);
        let distance = motion_math::cell_distance(self.player.position, target.position);
        if distance <= 2.0 && self.pending_network_steps.is_empty() {
            let now = self.started.elapsed().as_secs_f32();
            if now < self.next_attack_at {
                return;
            }
            if let (Some(network), Some(character_id)) = (&self.network, self.network_character_id)
            {
                let position = target_cell.map(|value| value as u16);
                network.send_attack(character_id, target_id, position[0], position[1]);
                self.face_towards(target_cell);
                // Keep the local request cadence aligned with the equipped
                // weapon while the server remains authoritative.
                self.next_attack_at =
                    now + shop::attack_cooldown_seconds(&mut self.assets, &self.equipment);
                self.network_status = format!("online: attacking monster {target_id}");
            }
            return;
        }
        if self.player.moving() {
            return;
        }
        // Stop at a walkable cell beside the monster. Walking onto the target
        // cell would make melee attacks look like ranged attacks and causes
        // actors to overlap.
        let candidates = [
            [target_cell[0] - 1, target_cell[1]],
            [target_cell[0] + 1, target_cell[1]],
            [target_cell[0], target_cell[1] - 1],
            [target_cell[0], target_cell[1] + 1],
        ];
        if let Some(destination) = candidates.into_iter().find(|cell| self.map.walkable(*cell)) {
            self.player.go(&self.map, destination);
        } else {
            self.pending_attack = None;
            self.network_status = format!("online: no path to monster {target_id}");
        }
    }

    fn skill_hud(&self) -> hud::SkillHud {
        let mut skills = self
            .known_skills
            .iter()
            .map(|(&id, &(level, experience))| (id, level, experience))
            .collect::<Vec<_>>();
        skills.sort_unstable();
        hud::SkillHud {
            skills,
            weapon_skills: self
                .weapon_skills
                .iter()
                .map(|(&weapon_type, &(level, experience))| (weapon_type, level, experience))
                .collect(),
            weapon_filter: self.weapon_filter,
            stamina: self.stamina,
            max_stamina: if self.status_flags & network::HEAVEN_BLESSING_FLAG != 0 {
                150
            } else {
                100
            },
            xp_ready: self.status_flags & network::XP_CIRCLE_FLAG != 0,
            bar_open: self.skill_bar_open,
            active_skill: self.active_skill,
            kill_count: self.kill_count,
            hovered: None,
        }
    }

    /// Right click in the world casts the skill selected in the skill bar,
    /// targeting the monster under the cursor when there is one.
    fn cast_active_skill_at_cursor(&mut self) -> bool {
        let Some(skill) = self.active_skill.and_then(action_bar::skill_action) else {
            return false;
        };
        if self.window.as_ref().is_some_and(|window| {
            let size = window.inner_size();
            hud::contains([size.width as f32, size.height as f32], self.cursor)
        }) {
            return false;
        }
        if skill.requires_target {
            let Some(target) = self.monster_at_cursor().or(self.selected_target) else {
                self.network_status = format!("selecione um monstro para usar {}", skill.name);
                return true;
            };
            self.selected_target = Some(target);
            self.pending_attack = None;
            self.pending_pickup = None;
            self.pending_skill_attack = Some((target, skill.id));
            self.process_pending_skill_attack();
        } else if action_bar::is_support_skill(skill.id)
            && let Some(ally) = self.player_at_cursor()
        {
            self.cast_support_skill(skill, ally);
        } else {
            self.cast_skill(skill);
        }
        true
    }

    /// A heal or revive aimed at another player.
    fn cast_support_skill(&mut self, skill: action_bar::SkillAction, target_id: u32) {
        let (Some(network), Some(character_id)) = (&self.network, self.network_character_id) else {
            return;
        };
        if !self.known_skills.contains_key(&skill.id) {
            self.network_status = format!("{} ainda não foi aprendida", skill.name);
            return;
        }
        let Some(target) = self.remote_actors.get(&target_id) else {
            return;
        };
        let cell = target.position.map(|value| value.round() as u16);
        network.send_magic_attack(character_id, target_id, cell[0], cell[1], skill.id);
        let now = self.started.elapsed().as_secs_f32();
        self.skill_cooldown_until.insert(skill.id, now + skill.cooldown_seconds);
        self.network_status = format!("online: {} -> {target_id}", skill.name);
    }

    /// Keeps attacking the target with the selected skill: walk until it is
    /// in range, then recast each time the cooldown expires.
    fn process_pending_skill_attack(&mut self) {
        let Some((target_id, skill_id)) = self.pending_skill_attack else {
            return;
        };
        let Some(skill) = action_bar::skill_action(skill_id) else {
            self.pending_skill_attack = None;
            return;
        };
        let Some(target) = self.remote_actors.get(&target_id) else {
            self.pending_skill_attack = None;
            return;
        };
        // Training dummies are NPCs without health; only real targets die.
        if !target.npc && (target.health == 0 || target.action == 160) {
            self.pending_skill_attack = None;
            return;
        }
        let target_cell = target.position.map(|value| value.round() as i32);
        let distance = motion_math::skill_distance(self.player.position, target.position);
        if distance > skill.range {
            if !self.player.moving() {
                let candidates = [
                    [target_cell[0] - 1, target_cell[1]],
                    [target_cell[0] + 1, target_cell[1]],
                    [target_cell[0], target_cell[1] - 1],
                    [target_cell[0], target_cell[1] + 1],
                ];
                if let Some(destination) =
                    candidates.into_iter().find(|cell| self.map.walkable(*cell))
                {
                    self.player.go(&self.map, destination);
                } else {
                    self.pending_skill_attack = None;
                    self.network_status = format!("online: no path to monster {target_id}");
                }
            }
            return;
        }
        if self.player.moving() {
            self.player.stop();
        }
        if !self.pending_network_steps.is_empty() {
            return;
        }
        let now = self.started.elapsed().as_secs_f32();
        if self
            .skill_cooldown_until
            .get(&skill.id)
            .is_some_and(|until| now < *until)
        {
            return;
        }
        self.selected_target = Some(target_id);
        self.cast_skill(skill);
    }

    /// Another player under the cursor (for heals and revives).
    fn player_at_cursor(&self) -> Option<u32> {
        let renderer = self.renderer.as_ref()?;
        let camera = renderer.camera(&self.map, &self.player);
        self.remote_actors
            .values()
            .filter(|actor| !actor.npc && !(400001..=499999).contains(&actor.id))
            .filter(|actor| {
                let projected = self.map.project(actor.position);
                let origin = [projected[0] - camera[0], projected[1] - camera[1]];
                (self.cursor[0] - origin[0]).abs() <= 32.0
                    && self.cursor[1] >= origin[1] - 120.0
                    && self.cursor[1] <= origin[1] + 24.0
            })
            .min_by(|left, right| {
                (left.position[0] + left.position[1])
                    .total_cmp(&(right.position[0] + right.position[1]))
            })
            .map(|actor| actor.id)
    }

    fn monster_at_cursor(&self) -> Option<u32> {
        let renderer = self.renderer.as_ref()?;
        let camera = renderer.camera(&self.map, &self.player);
        self.remote_actors
            .values()
            .filter(|actor| {
                // Summoned pets (495000..) fight for their owners.
                (!actor.npc && (400001..495_000).contains(&actor.id))
                    || (actor.npc && (is_training_goal(actor.npc_kind) || is_war_structure(actor.npc_kind)))
            })
            .filter(|actor| {
                let projected = self.map.project(actor.position);
                let origin = [projected[0] - camera[0], projected[1] - camera[1]];
                (self.cursor[0] - origin[0]).abs() <= 48.0
                    && self.cursor[1] >= origin[1] - 160.0
                    && self.cursor[1] <= origin[1] + 24.0
            })
            .min_by(|left, right| {
                (left.position[0] + left.position[1])
                    .total_cmp(&(right.position[0] + right.position[1]))
            })
            .map(|actor| actor.id)
    }

    fn cast_action_slot(&mut self, slot: usize) {
        let Some(skill_id) = self.action_bar.skill_id(slot) else {
            self.network_status = format!("action bar slot {} is empty", slot + 1);
            return;
        };
        let Some(skill) = action_bar::skill_action(skill_id) else {
            self.network_status = format!("skill {skill_id} ainda não possui ação no cliente");
            return;
        };
        self.cast_skill(skill);
    }

    fn cast_skill(&mut self, skill: action_bar::SkillAction) {
        let Some(network) = &self.network else {
            self.network_status = format!("{} indisponível: servidor offline", skill.name);
            return;
        };
        let Some(character_id) = self.network_character_id else {
            return;
        };
        if !self.known_skills.contains_key(&skill.id) {
            self.network_status = format!("{} ainda não foi aprendida", skill.name);
            return;
        }
        let now = self.started.elapsed().as_secs_f32();
        if self
            .skill_cooldown_until
            .get(&skill.id)
            .is_some_and(|until| now < *until)
        {
            self.network_status = format!("{} ainda está em cooldown", skill.name);
            return;
        }
        let (target_id, target_cell) = if skill.requires_target {
            let Some(target_id) = self.selected_target else {
                self.network_status = format!("selecione um monstro antes de usar {}", skill.name);
                return;
            };
            let Some(target) = self.remote_actors.get(&target_id) else {
                self.selected_target = None;
                self.network_status = "o alvo não está mais visível".into();
                return;
            };
            let target_cell = target.position.map(|value| value.round() as i32);
            let distance = motion_math::skill_distance(self.player.position, target.position);
            if distance > skill.range {
                self.network_status = format!("{}: o alvo está fora do alcance", skill.name);
                return;
            }
            (target_id, target_cell)
        } else {
            (character_id, self.player.cell())
        };
        let position = target_cell.map(|value| value as u16);
        network.send_magic_attack(character_id, target_id, position[0], position[1], skill.id);
        self.skill_cooldown_until
            .insert(skill.id, now + skill.cooldown_seconds);
        self.pending_attack = None;
        if skill.requires_target {
            self.face_towards(target_cell);
            self.network_status = format!("online: {} -> monster {target_id}", skill.name);
        } else {
            self.network_status = format!("online: {} ativada", skill.name);
        }
    }

    /// Stepping on a map portal (`DMap` portal list) asks the server to
    /// change map, like the original client's MsgAction 85.
    fn check_portal(&mut self) {
        if self.dead_since.is_some() {
            return;
        }
        let cell = self.player.cell();
        let portal = self
            .map
            .data
            .portals
            .iter()
            .find(|portal| (portal.x - cell[0]).abs() <= 1 && (portal.y - cell[1]).abs() <= 1)
            .copied();
        let Some(portal) = portal else {
            self.portal_armed = true;
            return;
        };
        if !self.portal_armed {
            return;
        }
        let (Some(network), Some(character_id)) = (&self.network, self.network_character_id) else {
            return;
        };
        self.portal_armed = false;
        network.change_map(
            character_id,
            portal.x.clamp(0, i32::from(u16::MAX)) as u16,
            portal.y.clamp(0, i32::from(u16::MAX)) as u16,
        );
        self.network_status = format!("online: portal at ({}, {})", portal.x, portal.y);
    }

    /// X: sit down or stand up (ChangeAction 250 / 100).
    fn toggle_sit(&mut self) {
        if self.dead_since.is_some() || self.player.moving() {
            return;
        }
        let (Some(network), Some(character_id)) = (&self.network, self.network_character_id) else {
            return;
        };
        self.sitting = !self.sitting;
        network.send_action(character_id, if self.sitting { 250 } else { 100 });
        self.player.action_instance = self.player.action_instance.wrapping_add(1);
        if !self.sitting {
            self.player.action = Action::Idle;
        }
    }

    /// M: start mining with a PickAxe in a mine.
    fn request_mining(&mut self) {
        if self.dead_since.is_some() {
            return;
        }
        if let (Some(network), Some(character_id)) = (&self.network, self.network_character_id) {
            network.mine(character_id);
            self.network_status = "online: mining".into();
        }
    }

    fn on_own_death(&mut self) {
        if self.dead_since.is_some() {
            return;
        }
        self.dead_since = Some(self.started.elapsed().as_secs_f32());
        self.player.stop();
        self.pending_attack = None;
        self.pending_skill_attack = None;
        self.pending_pickup = None;
        self.local_action = None;
        self.dialog = None;
        self.player.action_instance = self.player.action_instance.wrapping_add(1);
        self.network_status = "You are dead.".into();
    }

    fn on_own_revive(&mut self) {
        self.dead_since = None;
        if self
            .dialog
            .as_ref()
            .is_some_and(|dialog| dialog.task_id == REVIVE_TASK_ID)
        {
            self.dialog = None;
        }
        self.player.action = Action::Idle;
        self.player.action_instance = self.player.action_instance.wrapping_add(1);
    }

    /// Sends what was typed in the dialog's text box.
    fn submit_dialog_text(&mut self) {
        let Some(dialog) = self.dialog.take() else {
            return;
        };
        let Some((option_id, _)) = dialog.edit else {
            self.dialog = Some(dialog);
            return;
        };
        let text = dialog.input.trim().to_owned();
        if text.is_empty() {
            self.dialog = Some(dialog);
            return;
        }
        if let Some(network) = &self.network {
            network.send_npc_dialog_text(dialog.task_id, option_id, text);
            self.network_status = format!("online: NPC answer {option_id}");
        }
    }

    fn select_dialog_option(&mut self) -> bool {
        let Some(dialog) = self.dialog.as_ref() else {
            return false;
        };
        let Some(window) = &self.window else {
            return false;
        };
        let size = window.inner_size();
        let viewport = [size.width as f32, size.height as f32];
        let hit = npc_dialog::hit_test(viewport, self.cursor, &dialog.lines, &dialog.options)
            .unwrap_or(npc_dialog::DialogHit::Outside);
        match hit {
            npc_dialog::DialogHit::Outside => {
                self.dialog = None;
                self.network_status = "online: NPC dialog closed; moving".into();
                false
            }
            npc_dialog::DialogHit::Panel => true,
            npc_dialog::DialogHit::Close => {
                self.dialog = None;
                self.network_status = "online: NPC dialog closed".into();
                true
            }
            npc_dialog::DialogHit::Option(index) => {
                let Some(dialog) = self.dialog.as_ref() else {
                    return true;
                };
                let Some((option_id, _)) = dialog.options.get(index).cloned() else {
                    return true;
                };
                if dialog.edit.as_ref().is_some_and(|(edit_id, _)| *edit_id == option_id) {
                    if let Some(dialog) = self.dialog.as_mut() {
                        if dialog.input_focused && !dialog.input.trim().is_empty() {
                            self.submit_dialog_text();
                        } else {
                            dialog.input_focused = true;
                            dialog.refresh_edit_row();
                        }
                    }
                    return true;
                }
                if dialog.task_id == REVIVE_TASK_ID {
                    if let (Some(network), Some(character_id)) =
                        (&self.network, self.network_character_id)
                    {
                        network.revive(character_id);
                        self.network_status = "online: reviving".into();
                    }
                    self.dialog = None;
                    return true;
                }
                if let Some(network) = &self.network {
                    network.send_npc_dialog_answer(dialog.task_id, option_id);
                    self.dialog = None;
                    self.network_status = format!("online: NPC option {option_id}");
                }
                true
            }
        }
    }

    fn select_shop(&mut self) -> bool {
        if self.shop.is_none() {
            return false;
        }
        let Some(window) = &self.window else {
            return true;
        };
        let viewport = [
            window.inner_size().width as f32,
            window.inner_size().height as f32,
        ];
        if shop::close_at(viewport, self.cursor) {
            self.shop = None;
            self.network_status = "online: shop closed".into();
            return true;
        }
        let scroll = shop::scroll_direction_at(viewport, self.cursor);
        if let Some(scroll_down) = scroll {
            self.scroll_shop_page(scroll_down);
            return true;
        }
        if !shop::shop_panel_at(viewport, self.cursor) {
            self.shop = None;
            self.network_status = "online: shop closed; moving".into();
            return false;
        }
        let Some(shop) = &mut self.shop else {
            return true;
        };
        if let Some(index) = shop.item_at(viewport, self.cursor) {
            shop.selected = Some(index);
            let item_id = shop.items[index];
            self.dragging_shop_item = Some(item_id);
            self.network_status = format!(
                "online: drag item {item_id} to an empty inventory slot, or right-click to buy"
            );
        }
        true
    }

    fn scroll_shop_page(&mut self, scroll_down: bool) -> bool {
        let Some(window) = &self.window else {
            return false;
        };
        let size = window.inner_size();
        let viewport = [size.width as f32, size.height as f32];
        if !shop::shop_panel_at(viewport, self.cursor) {
            return false;
        }
        let Some(shop) = &mut self.shop else {
            return false;
        };
        let pages = shop.items.len().div_ceil(5).saturating_sub(3).max(1);
        let previous_page = shop.page;
        if scroll_down {
            shop.page = (shop.page + 1).min(pages.saturating_sub(1));
        } else {
            shop.page = shop.page.saturating_sub(1);
        }
        if shop.page != previous_page {
            shop.hover = None;
            shop.selected = None;
            self.network_status = format!(
                "online: shop scroll position {} of {}",
                shop.page + 1,
                pages
            );
        }
        true
    }

    fn finish_shop_drag(&mut self) -> bool {
        let Some(item_id) = self.dragging_shop_item.take() else {
            return false;
        };
        let Some(window) = &self.window else {
            return true;
        };
        let size = window.inner_size();
        let viewport = [size.width as f32, size.height as f32];
        let Some(slot) = shop::inventory_slot_at(viewport, self.cursor) else {
            self.network_status = "online: drop the item inside an inventory slot".into();
            return true;
        };
        if self.inventory.iter().any(|item| item.position == slot) {
            self.network_status = format!("online: inventory slot {} is occupied", slot + 1);
            return true;
        }
        self.request_shop_purchase(item_id, slot);
        true
    }

    fn apply_inventory_move_locally(
        &mut self,
        item: network::OwnedItem,
        slot: u8,
    ) -> Option<network::OwnedItem> {
        let swapped = self
            .inventory
            .iter()
            .find(|owned| owned.position == slot && owned.uid != item.uid)
            .copied();
        for owned in &mut self.inventory {
            if owned.uid == item.uid {
                owned.position = slot;
            } else if owned.position == slot {
                owned.position = item.position;
            }
        }
        self.inventory.sort_by_key(|owned| owned.position);
        swapped
    }

    fn restore_inventory_move(
        &mut self,
        moved: network::OwnedItem,
        swapped: Option<network::OwnedItem>,
    ) {
        for owned in &mut self.inventory {
            if owned.uid == moved.uid {
                owned.position = moved.position;
            } else if let Some(swapped) = swapped {
                if owned.uid == swapped.uid {
                    owned.position = swapped.position;
                }
            }
        }
        self.inventory.sort_by_key(|owned| owned.position);
    }

    fn advance_inventory_sort(&mut self) {
        if self.pending_inventory_sort_move.is_some() {
            return;
        }
        let Some(queue) = self.pending_inventory_sort.as_mut() else {
            return;
        };
        let Some(next) = queue.pop_front() else {
            self.pending_inventory_sort = None;
            self.selected_inventory_uid = None;
            self.network_status = "inventário organizado e confirmado pelo servidor".into();
            if let (Some(network), Some(character_id)) = (&self.network, self.network_character_id)
            {
                // A final snapshot repairs any updates that arrived while the
                // individual swap confirmations were being rendered.
                network.request_items(character_id);
            }
            if let Some(renderer) = &mut self.renderer {
                renderer.invalidate_inventory();
            }
            return;
        };
        self.pending_inventory_sort_move = Some(next);
        if let (Some(network), Some(character_id)) = (&self.network, self.network_character_id) {
            network.move_inventory_item(character_id, next.0, next.1);
            self.network_status = format!(
                "organizando inventário… posição {} de {}",
                next.1.saturating_add(1),
                self.pending_inventory_sort
                    .as_ref()
                    .map_or(1, |pending| pending.len().saturating_add(1))
            );
        } else {
            self.pending_inventory_sort = None;
            self.pending_inventory_sort_move = None;
        }
    }

    fn organize_inventory(&mut self) {
        if self.pending_inventory_sort.is_some() || self.pending_inventory_sort_move.is_some() {
            self.network_status = "aguarde o inventário terminar de organizar".into();
            return;
        }
        let mut desired = self
            .inventory
            .iter()
            .filter(|item| item.position < 40)
            .copied()
            .collect::<Vec<_>>();
        desired.sort_by_key(|item| (item.item_type, item.uid));

        // Build the same swap sequence the server will execute, but keep the
        // live online inventory untouched until authoritative responses arrive.
        let mut planned = self.inventory.clone();
        let mut moves = Vec::new();
        for (target, wanted) in desired.iter().copied().enumerate() {
            let target = target as u8;
            let Some(current) = planned.iter().find(|item| item.uid == wanted.uid).copied() else {
                continue;
            };
            if current.position == target {
                continue;
            }
            moves.push((current, target));
            for owned in &mut planned {
                if owned.uid == current.uid {
                    owned.position = target;
                } else if owned.position == target {
                    owned.position = current.position;
                }
            }
        }

        let moved_count = moves.len();
        if let (Some(_network), Some(_character_id)) = (&self.network, self.network_character_id) {
            let queue = moves
                .into_iter()
                .map(|(current, target)| (current.uid, target))
                .collect::<VecDeque<_>>();
            self.pending_inventory_sort = Some(queue);
            self.network_status = format!("organizando inventário: {moved_count} trocas pendentes");
            self.advance_inventory_sort();
        } else {
            for (current, target) in moves {
                self.apply_inventory_move_locally(current, target);
            }
            self.selected_inventory_uid = None;
            self.network_status = if moved_count == 0 {
                "inventário já está organizado".into()
            } else {
                format!("inventário organizado localmente: {moved_count} itens reposicionados")
            };
        }
        if let Some(renderer) = &mut self.renderer {
            renderer.invalidate_inventory();
        }
    }

    fn begin_inventory_drag(&mut self) -> bool {
        if self.shop.is_none() && self.warehouse.is_none() {
            let Some(window) = &self.window else {
                return false;
            };
            let size = window.inner_size();
            let viewport = [size.width as f32, size.height as f32];
            debug_log::write(format_args!(
                "begin standalone inventory: cursor=({:.1},{:.1}) viewport=({:.0},{:.0}) open={} panel_at={} close_at={} filter={:?} slot={:?}",
                self.cursor[0],
                self.cursor[1],
                viewport[0],
                viewport[1],
                self.inventory_open,
                shop::inventory_panel_at(viewport, self.cursor),
                shop::inventory_close_at(viewport, self.cursor),
                shop::inventory_filter_at(viewport, self.cursor),
                shop::inventory_slot_at(viewport, self.cursor),
            ));
            if !self.inventory_open || !shop::inventory_panel_at(viewport, self.cursor) {
                debug_log::write("begin ignored: cursor is outside the open inventory panel");
                return false;
            }
            if shop::inventory_close_at(viewport, self.cursor) {
                debug_log::write("begin handled by inventory close button");
                self.inventory_open = false;
                self.selected_inventory_uid = None;
                return true;
            }
            if shop::inventory_organize_at(viewport, self.cursor) {
                self.organize_inventory();
                return true;
            }
            if let Some(filter) = shop::inventory_filter_at(viewport, self.cursor) {
                debug_log::write(format_args!("begin handled by inventory filter {filter:?}"));
                self.inventory_filter = filter;
                self.selected_inventory_uid = None;
                return true;
            }
            let item = shop::inventory_slot_at(viewport, self.cursor).and_then(|slot| {
                self.inventory
                    .iter()
                    .find(|item| item.position == slot)
                    .copied()
            });
            self.dragging_inventory_item = item;
            self.inventory_drag_start_cursor = item.map(|_| self.cursor);
            self.selected_inventory_uid = item.map(|item| item.uid);
            if let Some(item) = item {
                debug_log::write(format_args!(
                    "drag started: uid={} type={} position={}",
                    item.uid, item.item_type, item.position
                ));
                self.network_status = format!("online: dragging item {}", item.item_type);
            } else {
                debug_log::write(
                    "begin handled by inventory panel, but no item exists in this slot",
                );
            }
            return true;
        }
        let Some(window) = &self.window else {
            return true;
        };
        let size = window.inner_size();
        let viewport = [size.width as f32, size.height as f32];
        debug_log::write(format_args!(
            "begin modal inventory: cursor=({:.1},{:.1}) panel_at={} filter={:?} slot={:?} warehouse={} shop={}",
            self.cursor[0],
            self.cursor[1],
            shop::inventory_panel_at(viewport, self.cursor),
            shop::inventory_filter_at(viewport, self.cursor),
            shop::inventory_slot_at(viewport, self.cursor),
            self.warehouse.is_some(),
            self.shop.is_some(),
        ));
        if !shop::inventory_panel_at(viewport, self.cursor) {
            return false;
        }
        if shop::inventory_organize_at(viewport, self.cursor) {
            self.organize_inventory();
            return true;
        }
        if let Some(filter) = shop::inventory_filter_at(viewport, self.cursor) {
            self.inventory_filter = filter;
            self.selected_inventory_uid = None;
            return true;
        }
        if let Some(slot) = shop::inventory_slot_at(viewport, self.cursor) {
            self.dragging_inventory_item = self
                .inventory
                .iter()
                .find(|item| item.position == slot)
                .copied();
            self.inventory_drag_start_cursor = self.dragging_inventory_item.map(|_| self.cursor);
            if self.warehouse.is_some() {
                self.selected_inventory_uid = self.dragging_inventory_item.map(|item| item.uid);
                self.selected_warehouse_uid = None;
            }
            if let Some(item) = self.dragging_inventory_item {
                debug_log::write(format_args!(
                    "drag started in modal: uid={} type={} position={}",
                    item.uid, item.item_type, item.position
                ));
                self.network_status = if self.warehouse.is_some() {
                    format!(
                        "online: drag item {} into the warehouse to store it",
                        item.item_type
                    )
                } else {
                    format!(
                        "online: drag item {} to the shop to sell, or right-click to sell",
                        item.item_type
                    )
                };
            }
        }
        true
    }

    fn begin_equipment_drag(&mut self) -> bool {
        if !self.status_open
            || self.status_tab != shop::StatusTab::Equipment
            || !self.inventory_open
            || self.shop.is_some()
            || self.warehouse.is_some()
        {
            return false;
        }
        let Some(window) = &self.window else {
            return false;
        };
        let size = window.inner_size();
        let viewport = [size.width as f32, size.height as f32];
        let Some(slot) = shop::status_equipment_slot_at(viewport, self.cursor) else {
            return false;
        };
        let item = self
            .equipment
            .iter()
            .find(|item| item.position == slot)
            .copied();
        let Some(item) = item else {
            return false;
        };
        debug_log::write(format_args!(
            "equipment drag started: uid={} type={} equipment_slot={}",
            item.uid, item.item_type, item.position
        ));
        self.dragging_equipment_item = Some(item);
        self.selected_inventory_uid = None;
        self.network_status = format!(
            "online: dragging equipped item {} to the inventory",
            item.item_type
        );
        true
    }

    fn inventory_slot_item_at_cursor(&self) -> Option<network::OwnedItem> {
        let window = self.window.as_ref()?;
        let size = window.inner_size();
        let viewport = [size.width as f32, size.height as f32];
        let slot = shop::inventory_slot_at(viewport, self.cursor)?;
        self.inventory
            .iter()
            .find(|item| item.position == slot)
            .copied()
    }

    fn equipment_slot_item_at_cursor(&self) -> Option<network::OwnedItem> {
        let window = self.window.as_ref()?;
        let size = window.inner_size();
        let viewport = [size.width as f32, size.height as f32];
        let slot = shop::status_equipment_slot_at(viewport, self.cursor)?;
        self.equipment
            .iter()
            .find(|item| item.position == slot)
            .copied()
    }

    /// The worn item under the cursor, including the character's default
    /// outfit (uid 0) that the paper doll shows in an empty slot.
    fn equipped_item_at_cursor(&self) -> Option<network::OwnedItem> {
        if let Some(item) = self.equipment_slot_item_at_cursor() {
            return Some(item);
        }
        let window = self.window.as_ref()?;
        let size = window.inner_size();
        let slot = shop::status_equipment_slot_at([size.width as f32, size.height as f32], self.cursor)?;
        shop::status_equipment_items(&self.character, &self.equipment, &self.hidden_default_equipment_slots)
            .into_iter()
            .find(|item| item.position == slot)
    }

    fn first_empty_inventory_slot(&self) -> Option<u8> {
        (0..40_u8).find(|slot| !self.inventory.iter().any(|item| item.position == *slot))
    }

    fn queue_equip_item(&mut self, item: network::OwnedItem) -> bool {
        if item.uid == 0 {
            debug_log::write(format_args!(
                "equip rejected locally: uid=0 type={} position={}",
                item.item_type, item.position
            ));
            self.network_status = "O outfit padrão não é um item do inventário".into();
            return true;
        }
        let Some(primary_slot) = shop::equipment_slot_for_item(&mut self.assets, item.item_type)
        else {
            debug_log::write(format_args!(
                "equip rejected locally: unsupported type={} uid={} position={}",
                item.item_type, item.uid, item.position
            ));
            self.network_status = "Este item não pode ser equipado".into();
            return true;
        };
        let equipment_slot = if primary_slot == 4
            && self.equipment.iter().any(|owned| owned.position == 4)
            && !self.equipment.iter().any(|owned| owned.position == 5)
        {
            5
        } else {
            primary_slot
        };
        if !shop::equipment_slot_accepts_item(&mut self.assets, item.item_type, equipment_slot) {
            debug_log::write(format_args!(
                "equip rejected locally: incompatible type={} uid={} slot={}",
                item.item_type, item.uid, equipment_slot
            ));
            self.network_status = "Este item não combina com esse slot".into();
            return true;
        }
        let (Some(network), Some(character_id)) = (&self.network, self.network_character_id) else {
            debug_log::write(format_args!(
                "equip rejected locally: offline type={} uid={} slot={}",
                item.item_type, item.uid, equipment_slot
            ));
            self.network_status = "Equipar itens requer o modo online".into();
            return true;
        };
        debug_log::write(format_args!(
            "equip requested: character_id={} uid={} type={} slot={} source_position={}",
            character_id, item.uid, item.item_type, equipment_slot, item.position
        ));
        network.equip_inventory_item(character_id, item.uid, equipment_slot);
        self.network_status = format!(
            "online: equipando item {} no slot {}",
            item.item_type, equipment_slot
        );
        true
    }

    fn queue_use_item(&mut self, item: network::OwnedItem) -> bool {
        if !shop::is_consumable(item.item_type) {
            self.network_status = "Este item ainda não pode ser usado".into();
            return true;
        }
        let (Some(network), Some(character_id)) = (&self.network, self.network_character_id) else {
            self.network_status = "Usar itens requer o modo online".into();
            return true;
        };
        network.use_item(character_id, item.uid);
        self.network_status = format!("online: usando item {}", item.item_type);
        true
    }

    fn queue_upgrade_item(&mut self, item: network::OwnedItem) -> bool {
        let (Some(network), Some(character_id)) = (&self.network, self.network_character_id) else {
            self.network_status = "Melhorar itens requer o modo online".into();
            return true;
        };
        network.upgrade_item(character_id, item.uid);
        self.network_status = format!("online: melhorando item {} com 1 Meteor", item.item_type);
        true
    }

    fn queue_improve_quality(&mut self, item: network::OwnedItem) -> bool {
        let (Some(network), Some(character_id)) = (&self.network, self.network_character_id) else {
            self.network_status = "Melhorar qualidade requer o modo online".into();
            return true;
        };
        network.improve_quality(character_id, item.uid);
        self.network_status = format!(
            "online: melhorando qualidade do item {} com DragonBall",
            item.item_type
        );
        true
    }

    fn queue_open_socket(&mut self, item: network::OwnedItem) -> bool {
        let (Some(network), Some(character_id)) = (&self.network, self.network_character_id) else {
            self.network_status = "Abrir socket requer o modo online".into();
            return true;
        };
        network.open_socket(character_id, item.uid);
        self.network_status = format!(
            "online: abrindo socket do item {} com DragonBall",
            item.item_type
        );
        true
    }

    fn queue_apply_refinery(&mut self, item: network::OwnedItem) -> bool {
        let target_slot = shop::equipment_slot_for_item(&mut self.assets, item.item_type);
        let Some(material) = self
            .inventory
            .iter()
            .find(|candidate| shop::refinery_material_slot(candidate.item_type) == target_slot)
            .copied()
        else {
            self.network_status = "Você não tem uma refinaria compatível no inventário".into();
            return true;
        };
        let Some(network) = &self.network else {
            self.network_status = "Aplicar refinaria requer o modo online".into();
            return true;
        };
        network.apply_refinery(item.uid, material.uid);
        self.network_status = format!(
            "online: aplicando refinaria {} no item {}",
            material.item_type, item.item_type
        );
        true
    }

    fn queue_apply_artifact(&mut self, item: network::OwnedItem) -> bool {
        let target_slot = shop::equipment_slot_for_item(&mut self.assets, item.item_type);
        let Some(material) = self
            .inventory
            .iter()
            .find(|candidate| shop::artifact_material_slot(candidate.item_type) == target_slot)
            .copied()
        else {
            self.network_status = "Você não tem um artefato compatível no inventário".into();
            return true;
        };
        let Some(network) = &self.network else {
            self.network_status = "Aplicar artefato requer o modo online".into();
            return true;
        };
        network.apply_artifact(item.uid, material.uid);
        self.network_status = format!(
            "online: aplicando artefato {} no item {}",
            material.item_type, item.item_type
        );
        true
    }

    fn queue_insert_gem(&mut self, item: network::OwnedItem, gem_uid: u32, socket: u8) -> bool {
        let Some(gem) = self
            .inventory
            .iter()
            .find(|owned| owned.uid == gem_uid && (700_000..800_000).contains(&owned.item_type))
            .copied()
        else {
            self.network_status = "Essa gem não está no inventário".into();
            return true;
        };
        let socket_open = match socket {
            1 => item.gem_one == 255,
            2 => item.gem_two == 255,
            _ => false,
        };
        if !socket_open {
            self.network_status = "Este item não tem socket aberto disponível".into();
            return true;
        }
        let Some(network) = &self.network else {
            self.network_status = "Inserir gem requer o modo online".into();
            return true;
        };
        network.insert_gem(item.uid, gem.uid, socket);
        self.network_status = format!(
            "online: inserindo gem {} no socket {}",
            gem.item_type, socket
        );
        true
    }

    fn queue_set_rarity(&mut self, item: network::OwnedItem, rarity: u8) -> bool {
        if rarity > 4 {
            self.network_status = "Raridade inválida".into();
            return true;
        }
        let Some(network) = &self.network else {
            self.network_status = "Alterar raridade requer o modo online".into();
            return true;
        };
        network.set_item_rarity(item.uid, rarity);
        self.network_status = format!(
            "online: alterando raridade de {} para {}",
            item.item_type,
            shop::rarity_label(rarity)
        );
        true
    }

    fn queue_reroll_rarity_bonus(&mut self, item: network::OwnedItem) -> bool {
        let Some(network) = &self.network else {
            self.network_status = "Rolar bônus requer o modo online".into();
            return true;
        };
        network.reroll_item_rarity(item.uid);
        self.network_status = format!("online: rolando bônus de {}", item.item_type);
        true
    }

    fn queue_remove_gem(&mut self, item: network::OwnedItem, socket: u8) -> bool {
        let occupied = match socket {
            1 => item.gem_one > 0 && item.gem_one != 255,
            2 => item.gem_two > 0 && item.gem_two != 255,
            _ => false,
        };
        if !occupied {
            self.network_status = "Este item não tem uma gem encaixada".into();
            return true;
        }
        let Some(network) = &self.network else {
            self.network_status = "Remover gem requer o modo online".into();
            return true;
        };
        network.remove_gem(item.uid, socket);
        self.network_status = format!("online: removendo gem do socket {}", socket);
        true
    }

    fn queue_unequip_item(&mut self, item: network::OwnedItem) -> bool {
        if item.uid == 0 {
            // The default outfit is only appearance: there is no item to
            // move to the inventory, so taking it off just hides it.
            debug_log::write(format_args!(
                "default outfit hidden: type={} slot={}",
                item.item_type, item.position
            ));
            self.hide_default_equipment_slot(item.position);
            self.refresh_character_equipment_model();
            self.network_status = "Aparência padrão removida (não é um item real)".into();
            return true;
        }
        let Some(inventory_slot) = self.first_empty_inventory_slot() else {
            debug_log::write(format_args!(
                "unequip rejected locally: no inventory space uid={} slot={}",
                item.uid, item.position
            ));
            self.network_status = "Não há espaço livre no inventário".into();
            return true;
        };
        let (Some(network), Some(character_id)) = (&self.network, self.network_character_id) else {
            debug_log::write(format_args!(
                "unequip rejected locally: offline uid={} slot={}",
                item.uid, item.position
            ));
            self.network_status = "Desequipar itens requer o modo online".into();
            return true;
        };
        debug_log::write(format_args!(
            "unequip requested: character_id={} uid={} equipment_slot={} inventory_slot={}",
            character_id, item.uid, item.position, inventory_slot
        ));
        network.unequip_item(character_id, item.uid, inventory_slot);
        // Remove the visual entry immediately. The server will confirm the
        // same change with action 9; retaining by UID/slot there keeps the
        // local and authoritative states convergent.
        self.equipment
            .retain(|owned| owned.uid != item.uid && owned.position != item.position);
        self.hide_default_equipment_slot(item.position);
        self.refresh_character_equipment_model();
        self.network_status = format!(
            "online: desequipando item {} para o slot {}",
            item.item_type,
            inventory_slot + 1
        );
        true
    }

    fn begin_item_context_menu(&mut self) -> bool {
        let Some(_window) = &self.window else {
            return false;
        };
        if self.shop.is_none()
            && self.warehouse.is_none()
            && self.inventory_open
            && let Some(item) = self.inventory_slot_item_at_cursor()
        {
            debug_log::write(format_args!(
                "context requested: inventory uid={} type={} position={} cursor=({:.1},{:.1})",
                item.uid, item.item_type, item.position, self.cursor[0], self.cursor[1]
            ));
            let actions = if shop::is_consumable(item.item_type) {
                vec![shop::ItemContextAction::Use]
            } else if shop::equipment_slot_for_item(&mut self.assets, item.item_type).is_some() {
                self.available_item_actions(item, false)
            } else {
                Vec::new()
            };
            if !actions.is_empty() {
                self.item_context = Some(shop::ItemContext {
                    item,
                    actions,
                    anchor: self.cursor,
                });
                self.network_status = format!("item {}: ação disponível", item.item_type);
            } else {
                self.network_status = "Este item não pode ser equipado".into();
            }
            return true;
        }
        if self.status_open
            && self.status_tab == shop::StatusTab::Equipment
            && let Some(item) = self.equipped_item_at_cursor()
        {
            // Like the original client, a right click on a worn item takes
            // it off.
            debug_log::write(format_args!(
                "equipment right click: uid={} type={} slot={}",
                item.uid, item.item_type, item.position
            ));
            self.item_context = None;
            return self.queue_unequip_item(item);
        }
        self.item_context = None;
        false
    }

    fn available_item_actions(
        &mut self,
        item: network::OwnedItem,
        equipped: bool,
    ) -> Vec<shop::ItemContextAction> {
        let mut actions = vec![if equipped {
            shop::ItemContextAction::Unequip
        } else {
            shop::ItemContextAction::Equip
        }];
        actions.push(shop::ItemContextAction::Upgrade);
        actions.push(shop::ItemContextAction::Rarity);

        // Classic quality upgrades only apply to Normal through Elite item
        // types. The final digit is independent from the custom rarity.
        if matches!(item.item_type % 10, 5..=8) {
            actions.push(shop::ItemContextAction::ImproveQuality);
        }
        // Keep both panels visible even when the required material is not in
        // the container; the panel explains what is missing instead of
        // making the action disappear from the item menu.
        actions.push(shop::ItemContextAction::ApplyRefinery);
        actions.push(shop::ItemContextAction::ApplyArtifact);

        actions.push(shop::ItemContextAction::GemSocket);
        actions
    }

    fn open_item_action_panel(
        &mut self,
        item: network::OwnedItem,
        action: shop::ItemContextAction,
    ) {
        if action == shop::ItemContextAction::GemSocket {
            let socket = if item.gem_one == 255 {
                1
            } else if item.gem_two == 255 {
                2
            } else {
                1
            };
            self.gem_selection_panel = Some(shop::GemSelectionPanel {
                item,
                socket,
                selected_gem: None,
            });
            self.network_status = "escolha o socket e a gem no inventário".into();
            return;
        }
        let selected_rarity = if action == shop::ItemContextAction::Rarity {
            item.rarity.saturating_add(1).min(4)
        } else {
            0
        };
        self.item_action_panel = Some(shop::ItemActionPanel {
            item,
            action,
            selected_rarity,
        });
        self.network_status = format!("{}: confirme a operação no painel", action.label());
    }

    fn handle_gem_selection_panel_click(&mut self) -> bool {
        let Some(panel) = self.gem_selection_panel else {
            return false;
        };
        let Some(window) = &self.window else {
            self.gem_selection_panel = None;
            return true;
        };
        let size = window.inner_size();
        let viewport = [size.width as f32, size.height as f32];
        if shop::gem_selection_panel_close_at(viewport, self.cursor) {
            self.gem_selection_panel = None;
            self.network_status = "seleção de gem cancelada".into();
            return true;
        }
        if let Some(action) = shop::gem_selection_panel_action_at(viewport, self.cursor, panel) {
            match action {
                shop::GemSelectionPanelButton::OpenSocket(socket) => {
                    if panel.item.gem_one == 0 && socket == 1
                        || panel.item.gem_two == 0 && socket == 2
                    {
                        self.gem_selection_panel = None;
                        self.queue_open_socket(panel.item);
                        self.network_status = format!("online: abrindo o socket {}", socket);
                    } else if let Some(gem_uid) = panel.selected_gem {
                        self.gem_selection_panel = None;
                        self.queue_insert_gem(panel.item, gem_uid, socket);
                    } else {
                        self.network_status = "selecione uma gem antes de clicar no botão +".into();
                    }
                }
                shop::GemSelectionPanelButton::RemoveGem(socket) => {
                    self.gem_selection_panel = None;
                    self.queue_remove_gem(panel.item, socket);
                }
                _ => {}
            }
            return true;
        }
        match shop::gem_selection_panel_button_at(viewport, self.cursor, &self.inventory) {
            Some(shop::GemSelectionPanelButton::Socket(socket)) => {
                if (socket == 1 && panel.item.gem_one == 255)
                    || (socket == 2 && panel.item.gem_two == 255)
                {
                    self.gem_selection_panel = Some(shop::GemSelectionPanel { socket, ..panel });
                }
            }
            Some(shop::GemSelectionPanelButton::Gem(gem_uid)) => {
                if panel.socket == 0 {
                    self.network_status = "selecione um socket aberto primeiro".into();
                } else {
                    self.gem_selection_panel = Some(shop::GemSelectionPanel {
                        selected_gem: Some(gem_uid),
                        ..panel
                    });
                    self.network_status =
                        "gem selecionada; clique no + do socket para inserir".into();
                }
            }
            Some(shop::GemSelectionPanelButton::OpenSocket(_))
            | Some(shop::GemSelectionPanelButton::RemoveGem(_)) => {}
            None => {}
        }
        true
    }

    fn handle_item_action_panel_click(&mut self) -> bool {
        let Some(panel) = self.item_action_panel else {
            return false;
        };
        if self.action_animation_started.is_some() {
            return true;
        }
        let Some(window) = &self.window else {
            self.item_action_panel = None;
            return true;
        };
        let size = window.inner_size();
        let viewport = [size.width as f32, size.height as f32];
        if shop::item_action_panel_close_at(viewport, self.cursor, panel.action) {
            self.item_action_panel = None;
            self.action_animation_started = None;
            self.network_status = "operação cancelada".into();
            return true;
        }
        match shop::item_action_panel_button_at(viewport, self.cursor, panel.action) {
            Some(shop::ItemActionPanelButton::Cancel) => {
                self.item_action_panel = None;
                self.action_animation_started = None;
            }
            Some(shop::ItemActionPanelButton::Reroll) => {
                if !shop::item_action_panel_reroll_enabled(panel, &self.inventory) {
                    self.network_status = if panel.item.rarity == 0 {
                        "o item precisa ser Rare ou melhor para rolar bônus".into()
                    } else {
                        "reroll indisponível: DragonBall insuficiente".into()
                    };
                    return true;
                }
                self.item_action_panel = None;
                self.queue_reroll_rarity_bonus(panel.item);
            }
            Some(shop::ItemActionPanelButton::Confirm) => {
                if !shop::item_action_panel_confirm_enabled(panel, &self.inventory) {
                    self.network_status = match panel.action {
                        shop::ItemContextAction::Upgrade if panel.item.plus >= 12 => {
                            "este item já está no nível máximo +12".into()
                        }
                        shop::ItemContextAction::Upgrade => {
                            "upgrade indisponível: Meteor insuficiente".into()
                        }
                        shop::ItemContextAction::Rarity if panel.item.rarity >= 4 => {
                            "este item já está na raridade máxima Mythic".into()
                        }
                        shop::ItemContextAction::Rarity => {
                            "raridade indisponível: DragonBalls insuficientes".into()
                        }
                        _ => "operação indisponível para este item".into(),
                    };
                    return true;
                }
                match panel.action {
                    shop::ItemContextAction::Upgrade => {
                        self.item_action_panel = Some(panel);
                        self.action_animation_started = Some(Instant::now());
                        self.queue_upgrade_item(panel.item)
                    }
                    shop::ItemContextAction::Rarity => {
                        self.item_action_panel = Some(panel);
                        self.action_animation_started = Some(Instant::now());
                        self.queue_set_rarity(panel.item, panel.selected_rarity)
                    }
                    shop::ItemContextAction::ImproveQuality => {
                        self.item_action_panel = None;
                        self.queue_improve_quality(panel.item)
                    }
                    shop::ItemContextAction::ApplyRefinery => {
                        self.item_action_panel = None;
                        self.queue_apply_refinery(panel.item)
                    }
                    shop::ItemContextAction::ApplyArtifact => {
                        self.item_action_panel = None;
                        self.queue_apply_artifact(panel.item)
                    }
                    shop::ItemContextAction::OpenSocket => {
                        self.item_action_panel = None;
                        self.queue_open_socket(panel.item)
                    }
                    shop::ItemContextAction::InsertGem => {
                        self.item_action_panel = None;
                        false
                    }
                    shop::ItemContextAction::RemoveGem => {
                        self.item_action_panel = None;
                        let socket = if panel.item.gem_one > 0 && panel.item.gem_one != 255 {
                            1
                        } else {
                            2
                        };
                        self.queue_remove_gem(panel.item, socket)
                    }
                    _ => false,
                };
            }
            None => {}
        }
        true
    }

    fn handle_item_context_click(&mut self) -> bool {
        let Some(context) = self.item_context.take() else {
            return false;
        };
        let Some(window) = &self.window else {
            return true;
        };
        let size = window.inner_size();
        let viewport = [size.width as f32, size.height as f32];
        if let Some(index) = shop::item_context_button_at(
            viewport,
            self.cursor,
            context.anchor,
            context.actions.len(),
        ) {
            match context.actions[index] {
                shop::ItemContextAction::Use => self.queue_use_item(context.item),
                shop::ItemContextAction::Equip => self.queue_equip_item(context.item),
                shop::ItemContextAction::Unequip => self.queue_unequip_item(context.item),
                shop::ItemContextAction::Upgrade
                | shop::ItemContextAction::Rarity
                | shop::ItemContextAction::ImproveQuality
                | shop::ItemContextAction::GemSocket
                | shop::ItemContextAction::OpenSocket
                | shop::ItemContextAction::ApplyRefinery
                | shop::ItemContextAction::ApplyArtifact
                | shop::ItemContextAction::InsertGem
                | shop::ItemContextAction::RemoveGem => {
                    self.open_item_action_panel(context.item, context.actions[index]);
                    false
                }
            };
        }
        true
    }

    fn handle_inventory_double_click(&mut self) -> bool {
        if !self.inventory_open || self.shop.is_some() || self.warehouse.is_some() {
            return false;
        }
        let Some(item) = self.inventory_slot_item_at_cursor() else {
            self.last_inventory_click = None;
            return false;
        };
        let now = Instant::now();
        let double = self.last_inventory_click.is_some_and(|(uid, at)| {
            uid == item.uid && now.duration_since(at) <= std::time::Duration::from_millis(350)
        });
        self.last_inventory_click = (!double).then_some((item.uid, now));
        if double {
            debug_log::write(format_args!(
                "inventory double click: uid={} type={} position={}",
                item.uid, item.item_type, item.position
            ));
            self.last_inventory_click = None;
            return if shop::is_consumable(item.item_type) {
                self.queue_use_item(item)
            } else {
                self.queue_equip_item(item)
            };
        }
        false
    }

    fn handle_equipment_double_click(&mut self) -> bool {
        if !self.status_open || self.status_tab != shop::StatusTab::Equipment {
            return false;
        }
        let Some(item) = self.equipped_item_at_cursor() else {
            self.last_equipment_click = None;
            return false;
        };
        let now = Instant::now();
        // The default outfit has no uid, so the slot identifies it.
        let key = if item.uid == 0 { u32::from(item.position) | 0x8000_0000 } else { item.uid };
        let double = self.last_equipment_click.is_some_and(|(clicked, at)| {
            clicked == key && now.duration_since(at) <= std::time::Duration::from_millis(350)
        });
        self.last_equipment_click = (!double).then_some((key, now));
        if double {
            debug_log::write(format_args!(
                "equipment double click: uid={} type={} slot={}",
                item.uid, item.item_type, item.position
            ));
            self.last_equipment_click = None;
            return self.queue_unequip_item(item);
        }
        false
    }

    fn finish_equipment_drag(&mut self) -> bool {
        let Some(item) = self.dragging_equipment_item.take() else {
            return false;
        };
        let Some(window) = &self.window else {
            return true;
        };
        let size = window.inner_size();
        let viewport = [size.width as f32, size.height as f32];
        let Some(slot) = shop::inventory_slot_at(viewport, self.cursor) else {
            self.network_status = "online: drop the equipment onto an inventory slot".into();
            return true;
        };
        if !self.inventory_open {
            self.network_status = "online: open the inventory to unequip an item".into();
            return true;
        }
        if let (Some(network), Some(character_id)) = (&self.network, self.network_character_id) {
            debug_log::write(format_args!(
                "equipment unequip queued: character_id={} uid={} equipment_slot={} inventory_slot={}",
                character_id, item.uid, item.position, slot
            ));
            network.unequip_item(character_id, item.uid, slot);
            self.equipment
                .retain(|owned| owned.uid != item.uid && owned.position != item.position);
            self.hide_default_equipment_slot(item.position);
            self.refresh_character_equipment_model();
            self.network_status = format!(
                "online: moving equipped item {} to slot {}",
                item.item_type,
                slot + 1
            );
        } else {
            self.network_status = "Unequipping items requires online mode".into();
        }
        true
    }

    fn finish_inventory_drag(&mut self) -> bool {
        let Some(item) = self.dragging_inventory_item.take() else {
            debug_log::write("drag release ignored: no inventory item was captured");
            return false;
        };
        let drag_start = self.inventory_drag_start_cursor.take();
        let Some(window) = &self.window else {
            return true;
        };
        let size = window.inner_size();
        let viewport = [size.width as f32, size.height as f32];
        // A normal click selects an item; it must never be interpreted as a
        // ground drop just because the release event arrived outside the
        // inventory hitbox. Only an intentional movement becomes a drag.
        if let Some([start_x, start_y]) = drag_start {
            let dx = self.cursor[0] - start_x;
            let dy = self.cursor[1] - start_y;
            if dx * dx + dy * dy <= 64.0 {
                self.selected_inventory_uid = Some(item.uid);
                self.network_status = format!("item selecionado: {}", item.item_type);
                return true;
            }
        }
        debug_log::write(format_args!(
            "drag release: uid={} type={} source_position={} cursor=({:.1},{:.1}) panel_at={} slot={:?} open={} shop={} warehouse={}",
            item.uid,
            item.item_type,
            item.position,
            self.cursor[0],
            self.cursor[1],
            shop::inventory_panel_at(viewport, self.cursor),
            shop::inventory_slot_at(viewport, self.cursor),
            self.inventory_open,
            self.shop.is_some(),
            self.warehouse.is_some(),
        ));
        if self.quick_panel == Some(shop::QuickPanel::Market)
            && self.player_shop_open
            && shop::quick_panel_at(viewport, self.cursor, shop::QuickPanel::Market)
        {
            if let (Some(network), Some(character_id)) = (&self.network, self.network_character_id)
            {
                let Some(price) = self
                    .player_shop_price
                    .parse::<u32>()
                    .ok()
                    .filter(|price| *price > 0)
                else {
                    self.network_status = "informe um preço maior que zero".into();
                    return true;
                };
                network.player_shop_add(character_id, item.uid, price);
                self.network_status = format!(
                    "online: anunciando item {} por {price} silver",
                    item.item_type
                );
            } else {
                self.network_status = "banca indisponível: servidor offline".into();
            }
            return true;
        }
        if self.quick_panel == Some(shop::QuickPanel::Trade)
            && shop::trade_panel_drop_at(viewport, self.cursor, self.trade.active)
        {
            if self
                .trade
                .own_items
                .iter()
                .any(|offered| offered.uid == item.uid)
            {
                self.network_status = "este item já está na oferta".into();
                return true;
            }
            let Some(network) = &self.network else {
                self.network_status = "trade indisponível: servidor offline".into();
                return true;
            };
            network.send_trade(item.uid, 6, 0);
            self.trade.own_items.push(item);
            self.pending_trade_items.push(item.uid);
            self.trade.own_confirmed = false;
            self.network_status = format!(
                "online: item {} oferecido por drag-and-drop",
                item.item_type
            );
            return true;
        }
        if self.status_open
            && self.status_tab == shop::StatusTab::Equipment
            && let Some(equipment_slot) = shop::status_equipment_slot_at(viewport, self.cursor)
        {
            if !shop::equipment_slot_accepts_item(&mut self.assets, item.item_type, equipment_slot)
            {
                self.network_status = "Este item não pode ser colocado nesse slot".into();
                return true;
            }
            if let (Some(network), Some(character_id)) = (&self.network, self.network_character_id)
            {
                network.equip_inventory_item(character_id, item.uid, equipment_slot);
                self.network_status = format!(
                    "online: equipping item {} in slot {}",
                    item.item_type, equipment_slot
                );
            } else {
                self.network_status = "Equipar itens requer o modo online".into();
            }
            self.selected_inventory_uid = Some(item.uid);
            return true;
        }
        if self.shop.is_none() && self.warehouse.is_none() && self.inventory_open {
            if let Some(slot) = shop::inventory_slot_at(viewport, self.cursor) {
                if slot == item.position {
                    self.selected_inventory_uid = Some(item.uid);
                    return true;
                }
                let swapped = self.apply_inventory_move_locally(item, slot);
                debug_log::write(format_args!(
                    "inventory move selected: uid={} from={} to={} swapped={:?}",
                    item.uid,
                    item.position,
                    slot,
                    swapped.map(|owned| (owned.uid, owned.position))
                ));
                if let (Some(network), Some(character_id)) =
                    (&self.network, self.network_character_id)
                {
                    self.pending_inventory_move = Some((item, swapped));
                    debug_log::write(format_args!(
                        "inventory move packet queued: character_id={} uid={} target_slot={}",
                        character_id, item.uid, slot
                    ));
                    network.move_inventory_item(character_id, item.uid, slot);
                    self.selected_inventory_uid = Some(item.uid);
                    self.network_status = format!(
                        "online: moving item {} to slot {}",
                        item.item_type,
                        slot + 1
                    );
                } else {
                    self.selected_inventory_uid = Some(item.uid);
                }
                return true;
            }
            if shop::inventory_panel_at(viewport, self.cursor) {
                debug_log::write(
                    "drag release rejected: inside inventory panel but outside a slot",
                );
                self.network_status = "online: drop the item inside an inventory slot".into();
                return true;
            }
        }
        if self.warehouse.is_some() && shop::warehouse_panel_at(viewport, self.cursor) {
            let occupied: std::collections::HashSet<u8> = self
                .warehouse
                .as_deref()
                .unwrap_or_default()
                .iter()
                .map(|owned| owned.position)
                .collect();
            let Some(slot) = shop::warehouse_slot_at(viewport, self.cursor, self.warehouse_page)
            else {
                self.network_status = "online: drop the item onto a warehouse slot".into();
                return true;
            };
            if occupied.contains(&slot) {
                self.network_status = "online: warehouse slot is occupied".into();
                return true;
            }
            if let (Some(network), Some(character_id), Some(npc_id)) = (
                &self.network,
                self.network_character_id,
                self.warehouse_npc_id,
            ) {
                if self.guild_warehouse_open {
                    network.guild_warehouse_deposit(character_id, npc_id, item.uid, slot);
                } else {
                    network.warehouse_deposit(character_id, npc_id, item.uid, slot);
                }
                self.selected_inventory_uid = Some(item.uid);
                self.network_status = format!("online: storing item {}", item.item_type);
            }
            return true;
        }
        if shop::inventory_panel_at(viewport, self.cursor) {
            return true;
        }
        if self.shop.is_none() && self.warehouse.is_none() {
            if let (Some(network), Some(character_id)) = (&self.network, self.network_character_id)
            {
                debug_log::write(format_args!(
                    "ground drop packet queued: character_id={} uid={} source_position={}",
                    character_id, item.uid, item.position
                ));
                self.inventory.retain(|owned| owned.uid != item.uid);
                self.pending_inventory_drop = Some(item);
                network.drop_inventory_item(character_id, item.uid);
                self.selected_inventory_uid = None;
                self.network_status =
                    format!("online: dropping item {} on the ground", item.item_type);
            } else {
                debug_log::write(format_args!(
                    "ground drop rejected locally: network={} character_id={:?}",
                    self.network.is_some(),
                    self.network_character_id
                ));
                self.network_status = "Dropping items requires online mode".into();
            }
            return true;
        }
        if !shop::shop_panel_at(viewport, self.cursor) {
            if self.warehouse.is_some() {
                self.network_status =
                    "online: drop the item inside the warehouse to store it".into();
                return true;
            }
            self.network_status = "online: drop the item inside the open shop to sell".into();
            return true;
        }
        let Some(shop) = &self.shop else {
            return true;
        };
        self.request_shop_sale(item.uid, shop.npc_id);
        true
    }

    fn sell_inventory_item_at_cursor(&mut self) -> bool {
        if self.shop.is_none() {
            return false;
        }
        let Some(window) = &self.window else {
            return true;
        };
        let size = window.inner_size();
        let viewport = [size.width as f32, size.height as f32];
        let Some(slot) = shop::inventory_slot_at(viewport, self.cursor) else {
            return false;
        };
        let Some(item) = self
            .inventory
            .iter()
            .find(|item| item.position == slot)
            .copied()
        else {
            return true;
        };
        let Some(shop) = &self.shop else {
            return true;
        };
        self.request_shop_sale(item.uid, shop.npc_id);
        true
    }

    fn warehouse_item_at_cursor(&mut self) -> bool {
        let Some(warehouse) = &self.warehouse else {
            return false;
        };
        let Some(window) = &self.window else {
            return true;
        };
        let size = window.inner_size();
        let viewport = [size.width as f32, size.height as f32];
        if let Some(forward) = shop::warehouse_page_at(viewport, self.cursor) {
            self.scroll_warehouse_page(forward);
            return true;
        }
        if shop::warehouse_close_at(viewport, self.cursor) {
            self.warehouse = None;
            self.warehouse_npc_id = None;
            self.guild_warehouse_open = false;
            self.selected_inventory_uid = None;
            self.selected_warehouse_uid = None;
            return true;
        }
        let Some(slot) = shop::warehouse_slot_at(viewport, self.cursor, self.warehouse_page) else {
            return shop::warehouse_panel_at(viewport, self.cursor);
        };
        let Some(item) = warehouse.iter().find(|item| item.position == slot).copied() else {
            self.selected_warehouse_uid = None;
            return true;
        };
        self.selected_warehouse_uid = Some(item.uid);
        self.selected_inventory_uid = None;
        let occupied: std::collections::HashSet<u8> =
            self.inventory.iter().map(|item| item.position).collect();
        let Some(inventory_slot) = (0u8..40).find(|candidate| !occupied.contains(candidate)) else {
            self.network_status = "online: inventory is full".into();
            return true;
        };
        if let (Some(network), Some(character_id), Some(npc_id)) = (
            &self.network,
            self.network_character_id,
            self.warehouse_npc_id,
        ) {
            if self.guild_warehouse_open {
                network.guild_warehouse_withdraw(character_id, npc_id, item.uid, inventory_slot);
            } else {
                network.warehouse_withdraw(character_id, npc_id, item.uid, inventory_slot);
            }
            self.network_status = format!("online: withdrawing item {}", item.item_type);
        }
        true
    }

    fn close_warehouse_at_cursor(&mut self) -> bool {
        if self.warehouse.is_none() {
            return false;
        }
        let Some(window) = &self.window else {
            return true;
        };
        let size = window.inner_size();
        if shop::warehouse_close_at([size.width as f32, size.height as f32], self.cursor) {
            self.warehouse = None;
            self.warehouse_npc_id = None;
            self.guild_warehouse_open = false;
            self.selected_inventory_uid = None;
            self.selected_warehouse_uid = None;
            true
        } else {
            false
        }
    }

    fn handle_warehouse_panel_click(&mut self) -> bool {
        if self.warehouse.is_none() {
            return false;
        }
        let Some(window) = &self.window else {
            return true;
        };
        let size = window.inner_size();
        let viewport = [size.width as f32, size.height as f32];
        if !self.guild_warehouse_open && shop::warehouse_amount_at(viewport, self.cursor) {
            self.warehouse_amount.clear();
            self.warehouse_amount_focused = true;
            return true;
        }
        self.warehouse_amount_focused = false;
        if let Some(deposit) = shop::warehouse_action_at(viewport, self.cursor) {
            if deposit {
                if let Some(uid) = self.selected_inventory_uid {
                    self.request_warehouse_deposit(uid, None);
                } else if self.guild_warehouse_open {
                    self.network_status =
                        "selecione um item do inventário para guardar na Guild".into();
                } else {
                    self.transfer_warehouse_silver(true);
                }
            } else if let Some(uid) = self.selected_warehouse_uid {
                self.request_warehouse_withdraw(uid);
            } else if self.guild_warehouse_open {
                self.network_status = "selecione um item do warehouse da Guild para retirar".into();
            } else {
                self.transfer_warehouse_silver(false);
            }
            return true;
        }
        if let Some(slot) = shop::warehouse_slot_at(viewport, self.cursor, self.warehouse_page) {
            if let Some(item) = self
                .warehouse
                .as_deref()
                .unwrap_or_default()
                .iter()
                .find(|item| item.position == slot)
            {
                self.selected_warehouse_uid = Some(item.uid);
                self.selected_inventory_uid = None;
            } else {
                self.selected_warehouse_uid = None;
            }
            return true;
        }
        shop::warehouse_panel_at(viewport, self.cursor)
    }

    fn transfer_warehouse_silver(&mut self, deposit: bool) {
        if self.guild_warehouse_open {
            self.network_status = "warehouse da Guild não armazena silver nesta etapa".into();
            return;
        }
        let amount = self.warehouse_amount.parse::<u32>().unwrap_or(0);
        if amount == 0 {
            self.network_status = "online: enter an amount greater than zero".into();
            return;
        }
        if let (Some(network), Some(character_id), Some(npc_id)) = (
            &self.network,
            self.network_character_id,
            self.warehouse_npc_id,
        ) {
            network.warehouse_silver(character_id, npc_id, amount, deposit);
            self.network_status = if deposit {
                format!("online: depositing {amount} silver")
            } else {
                format!("online: withdrawing {amount} silver")
            };
        }
    }

    fn scroll_warehouse_page(&mut self, down: bool) -> bool {
        if self.warehouse.is_none() {
            return false;
        }
        let Some(window) = &self.window else {
            return false;
        };
        let size = window.inner_size();
        let viewport = [size.width as f32, size.height as f32];
        if !shop::warehouse_panel_at(viewport, self.cursor) {
            return false;
        }
        self.warehouse_page = usize::from(down);
        self.selected_warehouse_uid = None;
        true
    }

    fn request_warehouse_deposit(&mut self, item_uid: u32, preferred_slot: Option<u8>) {
        let occupied: std::collections::HashSet<u8> = self
            .warehouse
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|item| item.position)
            .collect();
        let slot = preferred_slot
            .filter(|slot| !occupied.contains(slot))
            .or_else(|| (0u8..40).find(|slot| !occupied.contains(slot)));
        let Some(slot) = slot else {
            self.network_status = "online: warehouse is full".into();
            return;
        };
        if let (Some(network), Some(character_id), Some(npc_id)) = (
            &self.network,
            self.network_character_id,
            self.warehouse_npc_id,
        ) {
            if self.guild_warehouse_open {
                network.guild_warehouse_deposit(character_id, npc_id, item_uid, slot);
            } else {
                network.warehouse_deposit(character_id, npc_id, item_uid, slot);
            }
            self.selected_inventory_uid = Some(item_uid);
            self.network_status = "online: depositing item into warehouse".into();
        }
    }

    fn request_warehouse_withdraw(&mut self, item_uid: u32) {
        let occupied: std::collections::HashSet<u8> =
            self.inventory.iter().map(|item| item.position).collect();
        let Some(slot) = (0u8..40).find(|slot| !occupied.contains(slot)) else {
            self.network_status = "online: inventory is full".into();
            return;
        };
        if let (Some(network), Some(character_id), Some(npc_id)) = (
            &self.network,
            self.network_character_id,
            self.warehouse_npc_id,
        ) {
            if self.guild_warehouse_open {
                network.guild_warehouse_withdraw(character_id, npc_id, item_uid, slot);
            } else {
                network.warehouse_withdraw(character_id, npc_id, item_uid, slot);
            }
            self.selected_warehouse_uid = Some(item_uid);
            self.network_status = "online: withdrawing item from warehouse".into();
        }
    }

    fn deposit_inventory_item_at_cursor(&mut self) -> bool {
        let Some(window) = &self.window else {
            return true;
        };
        let size = window.inner_size();
        let viewport = [size.width as f32, size.height as f32];
        let Some(slot) = shop::inventory_slot_at(viewport, self.cursor) else {
            return false;
        };
        let Some(item) = self
            .inventory
            .iter()
            .find(|item| item.position == slot)
            .copied()
        else {
            return true;
        };
        let occupied: std::collections::HashSet<u8> = self
            .warehouse
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|item| item.position)
            .collect();
        let Some(warehouse_slot) = (0u8..40).find(|candidate| !occupied.contains(candidate)) else {
            self.network_status = "online: warehouse is full".into();
            return true;
        };
        if let (Some(network), Some(character_id), Some(npc_id)) = (
            &self.network,
            self.network_character_id,
            self.warehouse_npc_id,
        ) {
            if self.guild_warehouse_open {
                network.guild_warehouse_deposit(character_id, npc_id, item.uid, warehouse_slot);
            } else {
                network.warehouse_deposit(character_id, npc_id, item.uid, warehouse_slot);
            }
            self.network_status = format!("online: depositing item {}", item.item_type);
        }
        true
    }

    fn request_shop_sale(&mut self, item_uid: u32, shop_id: u32) {
        if let (Some(network), Some(character_id)) = (&self.network, self.network_character_id) {
            network.sell_item(character_id, item_uid, shop_id);
            self.network_status = format!("online: sale requested for item {item_uid}");
        } else {
            self.network_status = "Shop sales require online mode".into();
        }
    }

    fn buy_shop_item_at_cursor(&mut self) -> bool {
        let Some(current_shop) = &self.shop else {
            return false;
        };
        let Some(window) = &self.window else {
            return true;
        };
        let size = window.inner_size();
        let viewport = [size.width as f32, size.height as f32];
        let Some(index) = current_shop.item_at(viewport, self.cursor) else {
            return false;
        };
        let item_id = current_shop.items[index];
        let occupied: std::collections::HashSet<u8> =
            self.inventory.iter().map(|item| item.position).collect();
        let Some(slot) = (0u8..40).find(|slot| !occupied.contains(slot)) else {
            self.network_status = "online: inventory is full".into();
            return true;
        };
        self.request_shop_purchase(item_id, slot);
        true
    }

    fn request_shop_purchase(&mut self, item_id: u32, slot: u8) {
        if let (Some(network), Some(character_id)) = (&self.network, self.network_character_id) {
            network.buy_item(character_id, item_id, slot);
            self.network_status = format!(
                "online: buying item {item_id} into inventory slot {}",
                slot + 1
            );
        } else {
            self.network_status = "Shop purchases require online mode".into();
        }
    }

    fn send_network_action(&mut self, action: u32) {
        let Some(local_action) = wire_action(action) else {
            self.network_status = format!("online: unsupported action {action}");
            return;
        };
        let Some(network) = &self.network else {
            return;
        };
        let Some(character_id) = self.network_character_id else {
            return;
        };
        network.send_action(character_id, action);
        self.player.action = local_action;
        self.player.action_instance = self.player.action_instance.wrapping_add(1);
        let started_at = self.started.elapsed().as_secs_f32();
        self.local_action = Some((
            local_action,
            started_at,
            started_at + action_duration(action as u16, &self.character),
        ));
        self.network_status = format!("online: sending action {action}");
    }
}

fn wire_action(action: u32) -> Option<Action> {
    Some(match action {
        110 => Action::WalkLeft,
        111 => Action::WalkRight,
        120 => Action::RunLeft,
        121 => Action::RunRight,
        130 => Action::Jump,
        140 => Action::Attack,
        150 => Action::Hit,
        220 => Action::Pickup,
        _ => return None,
    })
}

fn action_duration(action: u16, character: &character::Character) -> f32 {
    match action {
        110 | 111 => 1.0 / 3.0,
        120 | 121 => 1.0 / 6.0,
        130 => character.duration(Action::Jump).as_secs_f32(),
        140 => character.duration(Action::Attack).as_secs_f32(),
        150 => character.duration(Action::Hit).as_secs_f32(),
        220 => character.duration(Action::Pickup).as_secs_f32(),
        250 => character.duration(Action::Sit).as_secs_f32(),
        _ => 0.5,
    }
}
/// `WeaponGoal`, `MagicGoal`, `BowGoal` and `Target`: training dummies.
fn is_training_goal(kind: u16) -> bool {
    (21..=24).contains(&kind)
}

/// The Guild War pole (`SynFlag`) and gates (`CityGate`).
fn is_war_structure(kind: u16) -> bool {
    matches!(kind, 10 | 26)
}

/// A large life shown on the u16 health bar: thousandths of the maximum.
fn scaled_life(life: u32, maximum: u32) -> u16 {
    if life == 0 {
        return 0;
    }
    ((u64::from(life) * 1_000 / u64::from(maximum.max(1))).clamp(1, 1_000)) as u16
}

fn run() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let mut root = None;
    let mut map_id = None;
    let mut body =
        sandbox_data::ini::SectionCatalog::parse(include_bytes!("../../../character.ini"))
            .section("Character")
            .and_then(|section| section.first("Body"))
            .unwrap_or("3")
            .parse::<u32>()?;
    let mut spawn = None;
    let mut check = false;
    let mut online = false;
    let mut auth_address = "127.0.0.1:9958".to_owned();
    let mut username = "test1".to_owned();
    let mut password = "123456".to_owned();
    let mut realm = "CoEmu".to_owned();
    let mut character_name = None;
    while let Some(arg) = args.next() {
        match arg.to_str().ok_or("Invalid argument encoding")? {
            "--assets" => {
                root = Some(PathBuf::from(
                    args.next().ok_or("--assets requires a directory")?,
                ))
            }
            "--map" => {
                map_id = Some(
                    args.next()
                        .ok_or("--map requires an ID")?
                        .to_str()
                        .ok_or("Invalid map")?
                        .parse()?,
                )
            }
            "--body" => {
                body = args
                    .next()
                    .ok_or("--body requires 1..4")?
                    .to_str()
                    .ok_or("Invalid body")?
                    .parse()?
            }
            "--spawn" => {
                let x = args
                    .next()
                    .ok_or("--spawn requires X Y")?
                    .to_str()
                    .ok_or("Invalid X")?
                    .parse()?;
                let y = args
                    .next()
                    .ok_or("--spawn requires X Y")?
                    .to_str()
                    .ok_or("Invalid Y")?
                    .parse()?;
                spawn = Some([x, y]);
            }
            "--check-assets" => check = true,
            "--online" => online = true,
            "--auth" => {
                auth_address = args
                    .next()
                    .ok_or("--auth requires an address")?
                    .to_str()
                    .ok_or("Invalid auth address")?
                    .to_owned();
                online = true;
            }
            "--username" => {
                username = args
                    .next()
                    .ok_or("--username requires a value")?
                    .to_str()
                    .ok_or("Invalid username")?
                    .to_owned();
            }
            "--password" => {
                password = args
                    .next()
                    .ok_or("--password requires a value")?
                    .to_str()
                    .ok_or("Invalid password")?
                    .to_owned();
            }
            "--realm" => {
                realm = args
                    .next()
                    .ok_or("--realm requires a value")?
                    .to_str()
                    .ok_or("Invalid realm")?
                    .to_owned();
            }
            "--character" => {
                character_name = Some(
                    args.next()
                        .ok_or("--character requires a name")?
                        .to_str()
                        .ok_or("Invalid character name")?
                        .to_owned(),
                );
            }
            "--help" | "-h" => {
                println!(
                    "You must supply your own Conquer Online 5065 installation.\ngravak-co --assets <installation> [--online|--auth HOST:PORT] [--username USER] [--password PASS] [--realm REALM] [--character NAME] [--map ID] [--spawn X Y] [--body 1..4] [--check-assets]\nOffline by default. In online mode the client authenticates and loads the server character/map. Click to move; Space stops; Escape exits. No bundled assets."
                );
                return Ok(());
            }
            _ => return Err("Unknown argument; use --help".into()),
        }
    }
    let mut network = None;
    let mut server_ready = None;
    if online {
        let handle = NetworkHandle::start(NetworkConfig {
            auth_address,
            username,
            password,
            realm,
            character_name,
        });
        let ready = handle.wait_ready()?;
        if map_id.is_none() {
            map_id = Some(ready.map_id);
        }
        server_ready = Some(ready);
        network = Some(handle);
    }
    let map_id = map_id.unwrap_or(1002);
    let mut assets = Assets::open(
        &root.ok_or("You must supply your own 5065 installation: --assets <directory>")?,
    )?;
    let npc_catalog = character::NpcCatalog::load(&mut assets)?;
    let map = map::Map::load(&mut assets, map_id)?;
    let mut character = character::Character::load(&mut assets, body)?;
    if let Some(ready) = &server_ready {
        if ready.body != 0
            // Start bare: the server's equipment and guild packets dress the
            // character. The sample profile's gear and guild are offline-only,
            // and other clients never see them.
            && let Ok(mut live_character) = character::Character::load_remote(
                &mut assets,
                ready.body,
                0,
                0,
                ready.hair_style,
                0,
                0,
                ready.character_name.clone(),
                ready.health.min(u16::MAX as u32) as u16,
            )
        {
            live_character.maximum_health = character
                .maximum_health
                .max(ready.maximum_health)
                .max(ready.health);
            character = live_character;
        }
        character.name = ready.character_name.clone();
        character.guild.clear();
        character.guild_rank.clear();
    }
    let requested_spawn = spawn;
    let preferred_spawn = requested_spawn
        .or_else(|| server_ready.as_ref().map(|ready| ready.position))
        .or(if map_id == 1002 {
            Some([430, 380])
        } else {
            None
        });
    let spawn = if requested_spawn.is_none() && server_ready.is_some() {
        preferred_spawn.and_then(|position| {
            let resolved = movement::nearest_walkable_spawn(&map, position, 24)
                .or(Some(position));
            if resolved != Some(position) {
                if let Some(corrected) = resolved {
                    eprintln!(
                        "Server spawn ({}, {}) is blocked locally; using nearby walkable cell ({}, {})",
                        position[0], position[1], corrected[0], corrected[1]
                    );
                }
            }
            resolved
        })
    } else {
        preferred_spawn
    };
    let player = movement::Player::new(&map, spawn)?;
    let mut audio = match audio::MovementAudio::load(&mut assets, &character) {
        Ok(audio) => Some(audio),
        Err(error) if check => return Err(error),
        Err(error) => {
            eprintln!("Audio unavailable: {error}");
            None
        }
    };
    if check {
        if let Some(audio) = &audio {
            audio.check()?;
        }
        for part in &character.parts {
            assets.image(&part.texture)?;
        }
        let remote_preview = character::Character::load_remote(
            &mut assets,
            1,
            130109,
            0,
            311,
            410339,
            410339,
            "RemotePreview".into(),
            100,
        )?;
        for part in &remote_preview.parts {
            assets.image(&part.texture)?;
        }
        let npc_preview = character::NpcModel::load(&mut assets, &npc_catalog, 2, 2610)?;
        assets.image(&npc_preview.texture)?;
        for (name, kind, look) in [
            ("Storekeeper", 1, 10),
            ("Warehouseman", 3, 80),
            ("Pedlar", 1, 2706),
        ] {
            let preview = character::NpcModel::load(&mut assets, &npc_catalog, kind, look)?;
            println!(
                "NPC asset audit: {name} look {look}, standby motion {}",
                if preview.has_animated_motion() {
                    "animated"
                } else if preview.has_motion() {
                    "loaded (static pose)"
                } else {
                    "missing"
                }
            );
        }
        let mut appearance_count = 0;
        let mut model_count = 0;
        let mut specific_model_count = 0;
        let mut motion_count = 0;
        let mut animated_count = 0;
        let mut missing_models = Vec::new();
        let mut generic_models = Vec::new();
        let mut missing_motions = Vec::new();
        let mut static_motions = Vec::new();
        for section in npc_catalog.appearance_sections() {
            let Some(look) = section
                .name
                .strip_prefix("NpcType")
                .and_then(|id| id.parse::<u16>().ok())
                .and_then(|id| id.checked_mul(10))
            else {
                continue;
            };
            appearance_count += 1;
            match character::NpcModel::load(&mut assets, &npc_catalog, 0, look) {
                Ok(model) => {
                    model_count += 1;
                    if model.uses_generic_fallback() {
                        if generic_models.len() < 10 {
                            generic_models.push(format!(
                                "{} ({})",
                                section.name,
                                section.first("Name").unwrap_or("unnamed")
                            ));
                        }
                    } else {
                        specific_model_count += 1;
                    }
                    if model.has_motion() {
                        motion_count += 1;
                        if model.has_animated_motion() {
                            animated_count += 1;
                        } else {
                            static_motions.push(format!(
                                "{} ({})",
                                section.name,
                                section.first("Name").unwrap_or("unnamed")
                            ));
                        }
                    } else if missing_motions.len() < 10 {
                        missing_motions.push(section.name.clone());
                    }
                }
                Err(error) if missing_models.len() < 10 => {
                    missing_models.push(format!("{} ({error})", section.name));
                }
                Err(_) => {}
            }
        }
        println!(
            "NPC catalogue audit: {appearance_count} appearances, {model_count} renderable models ({specific_model_count} specific, {} generic fallback), {motion_count} standby motions found, {animated_count} visibly animated",
            model_count - specific_model_count
        );
        if !missing_models.is_empty() {
            println!("Missing model examples: {}", missing_models.join(", "));
        }
        if !generic_models.is_empty() {
            println!("Generic model examples: {}", generic_models.join(", "));
        }
        if !missing_motions.is_empty() {
            println!("Missing motion examples: {}", missing_motions.join(", "));
        }
        if !static_motions.is_empty() {
            println!("Static-pose motion examples: {}", static_motions.join(", "));
        }
        validation::movement(&map, &character, player.cell())?;
        if let Some(minimap) = minimap::Minimap::load(&mut assets, map.id)? {
            let center = map.project(player.position);
            let quads = minimap.quads(
                &map,
                player.position,
                [center[0] - 683.0, center[1] - 384.0],
                [1366.0, 768.0],
            );
            if quads.len() != 4 {
                return Err("Minimap crop/hero/control check failed".into());
            }
            println!("Minimap OK: original assets, projected hero, crop and controls");
        }
        println!(
            "Assets OK: map {map_id}, {}x{}, {} sprites; local, remote and NPC appearances; character dek at {:?}",
            map.data.width,
            map.data.height,
            map.sprites.len(),
            player.cell()
        );
        return Ok(());
    }
    if let Some(audio) = &mut audio {
        audio.open_device();
    }
    let benchmark_screen = std::env::var("GRAVAK_BENCHMARK_SCREEN")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let benchmark_character = matches!(benchmark_screen.as_str(), "character" | "inventory");
    let benchmark_inventory = benchmark_screen == "inventory";
    let benchmark_shop = if benchmark_screen == "shop" {
        Some(shop::ShopState::load(&mut assets, 3)?.ok_or("benchmark shop 3 is missing")?)
    } else {
        // Keep the normal startup path free of Shop.dat/icon work.
        None
    };
    let benchmark_warehouse = benchmark_screen == "warehouse";
    let benchmark_guild = benchmark_screen == "guild";
    let frame_interval = configured_frame_interval();
    let remote_model_loader =
        match RemoteModelLoader::new(assets.root_path().to_owned(), npc_catalog.clone()) {
            Ok(loader) => Some(loader),
            Err(error) => {
                eprintln!("Remote model loader unavailable: {error}");
                None
            }
        };
    let mut app = App {
        assets,
        map,
        character,
        action_bar: action_bar::ActionBar::default(),
        action_context_slot: None,
        item_context: None,
        target_pick: None,
        pending_friend_request: None,
        cursors: HashMap::new(),
        shown_cursor: None,
        player_menu: None,
        follow_target: None,
        pk_menu_open: false,
        // The game server starts every character in peace mode.
        kill_mode: 1,
        item_action_panel: None,
        action_animation_started: None,
        gem_selection_panel: None,
        last_inventory_click: None,
        last_equipment_click: None,
        quick_panel: benchmark_guild.then_some(shop::QuickPanel::Guild),
        player,
        audio,
        window: None,
        renderer: None,
        cursor: [0.0; 2],
        started: Instant::now(),
        last: Instant::now(),
        error: None,
        modifiers: Default::default(),
        held: false,
        queued_jump: None,
        local_action: None,
        stamina: 100,
        dead_since: None,
        sitting: false,
        portal_armed: false,
        hit_until: 0.0,
        level_up_until: 0.0,
        level: server_ready.as_ref().map(|ready| ready.level).unwrap_or(1),
        initial_sync_complete: server_ready.is_some(),
        experience: server_ready
            .as_ref()
            .map(|ready| ready.experience)
            .unwrap_or_default(),
        nobility_donation: 0,
        nobility_rank: 0,
        nobility_remaining: 0,
        equipped_title: 0,
        nobility_ranking: Vec::new(),
        status_flags: 0,
        status_open: benchmark_character,
        status_tab: shop::StatusTab::Equipment,
        weapon_filter: shop::WeaponFilter::OneHand,
        performance_overlay: false,
        hud_config: hud::HudConfig::default(),
        network,
        network_character_id: server_ready.as_ref().map(|ready| ready.character_id),
        hidden_default_equipment_slots: [false; 10],
        equipment: Vec::new(),
        inventory: Vec::new(),
        inventory_open: benchmark_inventory,
        inventory_filter: shop::InventoryFilter::All,
        warehouse: benchmark_warehouse.then(Vec::new),
        warehouse_npc_id: benchmark_warehouse.then_some(3),
        guild_warehouse_open: false,
        warehouse_page: 0,
        selected_inventory_uid: None,
        selected_warehouse_uid: None,
        pending_inventory_move: None,
        pending_inventory_sort: None,
        pending_inventory_sort_move: None,
        pending_inventory_drop: None,
        silver: server_ready
            .as_ref()
            .map(|ready| ready.silver)
            .unwrap_or_default(),
        mana: server_ready
            .as_ref()
            .map(|ready| ready.mana)
            .unwrap_or_default(),
        maximum_mana: server_ready
            .as_ref()
            .map(|ready| ready.maximum_mana)
            .unwrap_or_default(),
        warehouse_silver: 0,
        cps: server_ready
            .as_ref()
            .map(|ready| ready.cps)
            .unwrap_or_default(),
        warehouse_amount: "1".into(),
        warehouse_amount_focused: false,
        chat_text: String::new(),
        chat_focused: false,
        chat_history: VecDeque::new(),
        chat_filter: 0,
        friends: Vec::new(),
        ignores: Vec::new(),
        dragging_shop_item: None,
        dragging_inventory_item: None,
        inventory_drag_start_cursor: None,
        dragging_equipment_item: None,
        confirmed_network_cell: server_ready.as_ref().map(|ready| ready.position),
        pending_network_steps: VecDeque::new(),
        pending_network_jump: false,
        pending_attack: None,
        pending_skill_attack: None,
        selected_target: None,
        pending_pickup: None,
        skill_cooldown_until: HashMap::new(),
        saved_hud: None,
        next_attack_at: 0.0,
        remote_actors: HashMap::new(),
        structure_lives: HashMap::new(),
        remote_models: HashMap::new(),
        remote_npcs: HashMap::new(),
        remote_monsters: HashMap::new(),
        combat_numbers: Vec::new(),
        skill_effects: Vec::new(),
        known_skills: HashMap::new(),
        weapon_skills: Default::default(),
        kill_count: 0,
        skill_bar_open: false,
        active_skill: None,
        team_members: Vec::new(),
        team_leader_id: None,
        pending_team_invite: None,
        pending_team_join_request: None,
        team_join_enabled: true,
        guild_members: Vec::new(),
        guild_id: None,
        pending_guild_invite: None,
        trade: network::TradeView::default(),
        pending_trade_items: Vec::new(),
        player_shop_items: Vec::new(),
        player_shop_open: false,
        player_shop_price: "1000".into(),
        player_shop_price_focused: false,
        ground_loot: Vec::new(),
        dialog: None,
        shop: benchmark_shop,
        network_status: if online {
            "online".into()
        } else {
            "offline".into()
        },
        frame_interval,
        next_frame_at: Instant::now(),
        remote_model_loader,
        remote_models_loading: HashSet::new(),
        remote_npcs_loading: HashSet::new(),
        remote_monsters_loading: HashSet::new(),
    };
    if let Some(saved) = app.network_character_id.and_then(settings::load) {
        saved.apply_to(&mut app.action_bar);
        app.skill_bar_open = saved.skill_bar_open;
        app.performance_overlay = saved.performance_overlay;
        app.team_join_enabled = saved.team_join_enabled;
        app.saved_hud = Some(saved);
    }
    EventLoop::new()?.run_app(&mut app)?;
    if let Some(error) = app.error {
        return Err(error.into());
    }
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("Sandbox: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::frame_interval_for;

    #[test]
    fn frame_pacing_supports_the_documented_fps_caps() {
        for fps in [60_u32, 100, 200, 300, 400, 500] {
            let interval = frame_interval_for(fps)
                .expect("a positive FPS cap should produce a frame interval")
                .as_secs_f64();
            let expected = 1.0 / f64::from(fps);
            assert!(
                (interval - expected).abs() < 1e-9,
                "fps={fps} interval={interval}"
            );
        }
        assert!(frame_interval_for(0).is_none());
    }
}

