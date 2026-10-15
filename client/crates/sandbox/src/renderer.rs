// Source: @digitalm1nd on x.com / _dek on Discord — Preservation Conquer project — https://discord.gg/CvKPXEHYRY
use crate::{
    action::Action,
    assets::{Assets, Result},
    character::Character,
    map::Map,
    movement::Player,
    network::RemoteActor,
};
use sandbox_data::c3::{C3Document, Matrix4, TransparencyKey};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
    sync::mpsc::{self, Receiver, Sender},
    thread,
};
use web_time::{Duration, Instant};
use wgpu::util::DeviceExt;
use winit::window::Window;
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    uv: [f32; 2],
    alpha: f32,
}
/// Sort depth of a UI model preview outside the status paper doll (the
/// trade partner card) and of what it stands on: above the world like the
/// UI, but without the status panel's scissor that INFINITY depth-tested
/// draws get, and before the INFINITY UI layered over it.
const PREVIEW_SORT: f32 = f32::MAX;
/// A crypt (`crate::reveal`): the dark over the cells that have not come
/// out, its picture (a cell's diamond) and sort (over the ground, under what
/// stands on it).
const DARK_CELL: &str = "@crypt-dark-cell";
const DARK_SORT: f32 = -1.0e30;

/// A cell's diamond in black, for the dark of a crypt.
fn dark_cell_image() -> image::RgbaImage {
    image::RgbaImage::from_fn(68, 36, |x, y| {
        let dx = ((x as f32 + 0.5) - 34.0).abs() / 34.0;
        let dy = ((y as f32 + 0.5) - 18.0).abs() / 18.0;
        image::Rgba([0, 0, 0, if dx + dy <= 1.0 { 255 } else { 0 }])
    })
}

/// How tall a hero stands on screen, and how much of a crypt's wall in
/// front of them, over them, still shows.
const HERO_SEEN_HEIGHT: f32 = 110.0;
const WALL_SEEN_THROUGH: f32 = 0.35;

/// Sort depth of what floats over the world (names, nameplates, damage
/// numbers, ground loot labels): above every world draw but below the UI,
/// the trade partner card included, so a name never shows through a window.
const WORLD_OVERLAY_SORT: f32 = f32::MAX / 2.0;

/// How high a rank's fiery name plays over the hero's feet: above the
/// name (152) and the VIP title over it.
const NOBILITY_LETTER_LIFT: f32 = 200.0;

/// Where a noble's insignia plays: just right of their name.
fn insignia_beside_name(origin: [f32; 2], name: &str) -> [f32; 2] {
    let half = crate::labels::name_width(name) * 0.5;
    [origin[0] + half + 16.0, origin[1] - 144.0]
}


struct Draw {
    texture: String,
    vertices: Vec<Vertex>,
    depth_test: bool,
    sort_depth: f32,
}

struct Texture {
    bind: wgpu::BindGroup,
    size: [f32; 2],
}
struct LevelUpLayer {
    document: C3Document,
    texture: String,
}

/// A cast seen on screen: the caster's intone effect and the target effect
/// from `MagicType.dat`, played from `started`.
#[derive(Debug, Clone, PartialEq)]
pub struct SkillEffectInstance {
    pub attacker_id: u32,
    pub target_id: u32,
    pub skill_id: u32,
    pub level: u16,
    pub started: f32,
    pub expires: f32,
    /// The cell the cast was aimed at, for its trail.
    pub target_cell: (u16, u16),
    /// Whether this one draws the cast's trail (the first of its targets).
    pub trail: bool,
}

/// A slain monster's soul: it rises over the body, then flies to the one
/// who slew it, a comet of mint light bending to one side, its trail long
/// behind it (`push_soul`).
#[derive(Debug, Clone, Copy)]
struct Soul {
    /// Where the monster fell, in cells.
    from: [f32; 2],
    killer_id: u32,
    started: f32,
}

/// An arrow in flight from an archer to the target, as `3DFlyingObj.ini`
/// shows it.
#[derive(Debug, Clone)]
struct Arrow {
    attacker_id: u32,
    target_id: u32,
    /// Whether the archer or the target is the player, who is not among
    /// the remote actors.
    from_player: bool,
    to_player: bool,
    flying: crate::magic::FlyingObject,
    started: f32,
}

/// The arrow leaves the bow this long into the shot, flies this many cells
/// a second, from about the hands to about the chest (pixels above the
/// feet), drawn at this size of the models.
/// As the original's arrows (`Conquer.exe` 0x4a23c8, 0x4a251b): loosed
/// 800 ms after the shot (the bow drawn), then 1600 pixels a second in a
/// straight line, from 25 pixels ahead of the archer 60 above the ground
/// to 60 above the target.
const ARROW_RELEASE_SECONDS: f32 = 0.8;
const ARROW_PIXELS_PER_SECOND: f32 = 1_600.0;
const ARROW_LEAD: f32 = 25.0;
const ARROW_FROM_LIFT: f32 = 60.0;
const ARROW_TO_LIFT: f32 = 60.0;
const ARROW_SCALE: f32 = 0.6;
/// How fast a skill's projectile (`MagicType` fly effect) crosses the map.
const SPELL_CELLS_PER_SECOND: f32 = 14.0;

/// How long a soul rises over the body, then how long it flies (faster
/// than the original's 1 and 1.6: ours streak in).
const SOUL_RISE_SECONDS: f32 = 0.5;
const SOUL_FLIGHT_SECONDS: f32 = 1.3;
/// How much of a soul's way its trail covers behind it.
const SOUL_TRAIL: f32 = 0.5;
/// The soul's light: mint, white at its heart.
const SOUL_TINT: [f32; 3] = [0.45, 1.0, 0.68];

/// The original `born` effect (`3DEffect.ini`) over a respawning monster:
/// 151 frames of 40 ms. The monster itself fades in over the first moments.
pub const MONSTER_BORN_SECONDS: f32 = 6.04;
const MONSTER_FADE_IN_SECONDS: f32 = 1.2;

/// The server's VIP status flag (a Gravak runtime bit).
pub const VIP_FLAG: u64 = 1 << 46;

/// Upper bound kept by the client; each effect ends with its own animation.
pub const SKILL_EFFECT_MAX_SECONDS: f32 = 4.0;

/// An original 3D effect (`3DEffect.ini`) loaded into C3 layers.
/// `ini/statuseffect.ini`: status flag and its looping effect.
struct LoadedEffect {
    layers: Vec<LevelUpLayer>,
    frame_interval: f32,
    frames: f32,
}

#[derive(Clone)]
struct StaticMapSprite {
    texture: String,
    origin: [f32; 2],
    depth: f32,
    cell: Option<[i32; 2]>,
}

struct StaticMapCache {
    map_id: u32,
    sprite_count: usize,
    static_sprites: Vec<StaticMapSprite>,
    dynamic_indices: Vec<usize>,
    grid: HashMap<(i32, i32), Vec<usize>>,
}

struct StaticVisibilityCache {
    map_id: u32,
    sprite_count: usize,
    viewport: [u32; 2],
    camera_bin: [i32; 2],
    indices: Arc<Vec<usize>>,
}

const STATIC_GRID_SIZE: f32 = 256.0;
const STATIC_GROUND_MARGIN: f32 = 256.0;
const STATIC_OBJECT_MARGIN: f32 = 1024.0;
/// Textures asked of the loader in a frame: enough for a screen of ground
/// in a few frames, so a jump's landing is not drawn black meanwhile.
const TEXTURE_REQUESTS_PER_FRAME: usize = 24;
/// How long a frame keeps putting loaded textures on the GPU, past the first
/// `max_texture_loads_per_frame`.
const TEXTURE_UPLOAD_TIME: Duration = Duration::from_millis(2);

fn static_grid_cell(position: [f32; 2]) -> (i32, i32) {
    (
        (position[0] / STATIC_GRID_SIZE).floor() as i32,
        (position[1] / STATIC_GRID_SIZE).floor() as i32,
    )
}

struct LoadedTexture {
    path: String,
    data: LoadedData,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum GeometryKey {
    Npc((u32, i8)),
    Character((u64, i8, usize)),
}

#[derive(Clone)]
enum GeometryJob {
    Npc {
        key: GeometryKey,
        model: crate::character::NpcModel,
        facing: f32,
    },
    Character {
        key: GeometryKey,
        character: Character,
        part_index: usize,
        facing: f32,
    },
}

struct GeometryResult {
    key: GeometryKey,
    vertices: Arc<Vec<([f32; 3], [f32; 2])>>,
}

struct GeometryLoader {
    requests: crate::background::Requests<GeometryJob>,
    results: crate::background::Results<GeometryResult>,
}

fn appearance_signature(character: &Character) -> u64 {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    character.body_id.hash(&mut hasher);
    character.armor_id.hash(&mut hasher);
    character.helmet_id.hash(&mut hasher);
    character.hair_id.hash(&mut hasher);
    character.right_weapon_id.hash(&mut hasher);
    character.left_weapon_id.hash(&mut hasher);
    character.weapon_action.hash(&mut hasher);
    for part in &character.parts {
        part.texture.hash(&mut hasher);
        part.mesh.vertices.len().hash(&mut hasher);
    }
    hasher.finish()
}

enum LoadedData {
    Image(std::result::Result<image::RgbaImage, String>),
    /// A panel redraw dropped for a newer one of the same panel.
    Superseded,
    Minimap {
        map_id: u32,
        result: std::result::Result<Option<crate::minimap::Minimap>, String>,
    },
}

struct TextureLoader {
    requests: crate::background::Requests<String>,
    results: crate::background::Results<LoadedTexture>,
}

#[cfg(not(target_arch = "wasm32"))]
struct UiTextureLoader {
    requests: Sender<UiTextureJob>,
    results: Receiver<LoadedTexture>,
}

#[cfg(target_arch = "wasm32")]
struct UiTextureLoader {
    requests: crate::background::Requests<UiTextureJob>,
    results: crate::background::Results<LoadedTexture>,
}

#[derive(Clone)]
enum UiTextureJob {
    Minimap {
        path: String,
        map_id: u32,
    },
    Shadow {
        path: String,
    },
    Label {
        path: String,
        character: Character,
        name_colour: [u8; 3],
    },
    Feedback {
        path: String,
        message: String,
    },
    ActorLabel {
        path: String,
        name: String,
        guild: String,
        guild_rank: String,
        shield: u8,
        monster: bool,
        /// A monster of the bounty under way.
        bounty: bool,
        health: u16,
        maximum_health: u16,
        name_colour: [u8; 3],
    },
    Hud {
        path: String,
        width: u32,
        height: u32,
        character_name: String,
        health: u32,
        max_health: u32,
        mana: u32,
        max_mana: u32,
        level: u8,
        experience: u64,
        action_bar: crate::action_bar::ActionBar,
        hover: Option<u8>,
        action_context_slot: Option<usize>,
        chat_text: String,
        chat_focused: bool,
        chat_history: Vec<crate::network::ChatMessage>,
        chat_filter: u16,
        double_experience_active: bool,
        heaven_blessing_active: bool,
        burning_active: bool,
        stunned_active: bool,
        cyclone_active: bool,
        superman_active: bool,
        stigma_active: bool,
        shield_active: bool,
        hud_config: crate::hud::HudConfig,
        skills: crate::hud::SkillHud,
    },
    StatusBase {
        path: String,
        active_tab: crate::shop::StatusTab,
        weapon_filter: crate::shop::WeaponFilter,
        close_hovered: bool,
        attribute_details: bool,
    },
    StatusDynamic {
        path: String,
        level: u8,
        experience: u64,
        nobility_rank: u32,
        character: Character,
        active_tab: crate::shop::StatusTab,
        mana: u32,
        maximum_mana: u32,
        known_skills: Vec<(u32, u16, u32)>,
        equipment: Vec<crate::network::OwnedItem>,
        weapon_skills: Vec<(u32, u32, u32)>,
        weapon_filter: crate::shop::WeaponFilter,
        skill_page: crate::shop::SkillPage,
        attribute_details: bool,
    },
    StatusOverlay {
        path: String,
        equipment: Vec<crate::network::OwnedItem>,
        hovered_slot: Option<u8>,
    },
    InventoryBase {
        path: String,
    },
    InventoryOverlay {
        path: String,
        inventory: Vec<crate::network::OwnedItem>,
        silver: u32,
        cps: u32,
        selected_uid: Option<u32>,
        filter: crate::shop::InventoryFilter,
        hovered_filter: Option<crate::shop::InventoryFilter>,
        close_hovered: bool,
        organize_hovered: bool,
    },
    QuickPanel {
        path: String,
        panel: crate::shop::QuickPanel,
        character: Character,
        action_bar: crate::action_bar::ActionBar,
        known_skills: std::collections::HashMap<u32, (u16, u32)>,
        team_members: Vec<crate::network::TeamMember>,
        team_leader_id: Option<u32>,
        pending_invite: Option<u32>,
        pending_join_request: Option<u32>,
        team_join_enabled: bool,
        guild_members: Vec<crate::network::GuildMember>,
        pending_guild_invite: Option<u32>,
        friends: Vec<crate::network::SocialEntry>,
        ignores: Vec<crate::network::SocialEntry>,
        pending_friend_name: Option<String>,
        selected_target_name: Option<String>,
        inventory: Vec<crate::network::OwnedItem>,
        trade: crate::network::TradeView,
        booth: crate::booth::BoothView,
        own_character_id: Option<u32>,
        hud_config: crate::hud::HudConfig,
        bounty: Option<crate::bounty_board::BountyView>,
        store: Option<crate::store_panel::StoreView>,
        daily: Option<crate::daily_rewards_panel::DailyRewardsView>,
        highscores: Option<crate::highscores_panel::HighscoresView>,
        nobility: Option<crate::nobility_panel::NobilityView>,
        guild: Option<crate::guild_panel::GuildView>,
    },
    ShopBase {
        path: String,
    },
    ShopOverlay {
        path: String,
        state: crate::shop::ShopState,
    },
    WarehouseBase {
        path: String,
        guild_warehouse: bool,
    },
    WarehouseOverlay {
        path: String,
        items: Vec<crate::network::OwnedItem>,
        page: usize,
        selected_uid: Option<u32>,
        saved_silver: u32,
        amount_text: String,
        amount_focused: bool,
        guild_warehouse: bool,
    },
}

impl UiTextureJob {
    /// Whole-panel redraws: only the newest of a kind is worth drawing, the
    /// ones queued before it are already out of date.
    fn panel(&self) -> Option<u8> {
        Some(match self {
            Self::Hud { .. } => 0,
            Self::StatusDynamic { .. } => 1,
            Self::StatusOverlay { .. } => 2,
            Self::InventoryOverlay { .. } => 3,
            Self::QuickPanel { .. } => 4,
            Self::ShopOverlay { .. } => 5,
            Self::WarehouseOverlay { .. } => 6,
            _ => return None,
        })
    }

    fn path(&self) -> &str {
        match self {
            Self::Minimap { path, .. }
            | Self::Shadow { path, .. }
            | Self::Label { path, .. }
            | Self::Feedback { path, .. }
            | Self::ActorLabel { path, .. }
            | Self::Hud { path, .. }
            | Self::StatusBase { path, .. }
            | Self::StatusDynamic { path, .. }
            | Self::StatusOverlay { path, .. }
            | Self::InventoryBase { path, .. }
            | Self::InventoryOverlay { path, .. }
            | Self::QuickPanel { path, .. }
            | Self::ShopBase { path, .. }
            | Self::ShopOverlay { path, .. }
            | Self::WarehouseBase { path, .. }
            | Self::WarehouseOverlay { path, .. } => path,
        }
    }

    /// What the job makes: a minimap, or a picture.
    fn load(self, assets: &mut Assets) -> LoadedData {
        match self {
            UiTextureJob::Minimap { map_id, .. } => LoadedData::Minimap {
                map_id,
                result: crate::minimap::Minimap::load(assets, map_id).map_err(|error| error.to_string()),
            },
            job => LoadedData::Image(job.render(assets)),
        }
    }

    fn render(self, assets: &mut Assets) -> std::result::Result<image::RgbaImage, String> {
        let image = match self {
            Self::Minimap { .. } => {
                return Err("minimap jobs are rendered by the loader worker".to_owned());
            }
            Self::Shadow { .. } => {
                const SHADOW: &str = "data/pic/Shadow/simpleShadow.dds";
                let shadow = assets.image(SHADOW).map_err(|error| error.to_string())?;
                let mut min = [shadow.width(), shadow.height()];
                let mut max = [0, 0];
                for (x, y, pixel) in shadow.enumerate_pixels() {
                    if pixel[3] != 0 {
                        min[0] = min[0].min(x);
                        min[1] = min[1].min(y);
                        max[0] = max[0].max(x);
                        max[1] = max[1].max(y);
                    }
                }
                if max[0] < min[0] || max[1] < min[1] {
                    return Err("shadow asset has no visible pixels".to_owned());
                }
                Ok(image::imageops::crop_imm(
                    &shadow,
                    min[0],
                    min[1],
                    max[0] - min[0] + 1,
                    max[1] - min[1] + 1,
                )
                .to_image())
            }
            Self::Label {
                character,
                name_colour,
                ..
            } => crate::labels::image(&character, name_colour),
            Self::Feedback { message, .. } => crate::labels::status_image(&message),
            Self::ActorLabel {
                name,
                guild,
                guild_rank,
                shield,
                monster,
                bounty,
                health,
                maximum_health,
                name_colour,
                ..
            } => {
                if monster {
                    crate::labels::monster_image(&name, name_colour, health, maximum_health, bounty)
                } else {
                    crate::labels::actor_image_with_guild(&name, name_colour, &guild, &guild_rank, shield)
                }
            }
            Self::Hud {
                width,
                height,
                character_name,
                health,
                max_health,
                mana,
                max_mana,
                level,
                experience,
                action_bar,
                hover,
                action_context_slot,
                chat_text,
                chat_focused,
                chat_history,
                chat_filter,
                double_experience_active,
                heaven_blessing_active,
                burning_active,
                stunned_active,
                cyclone_active,
                superman_active,
                stigma_active,
                shield_active,
                hud_config,
                skills,
                ..
            } => crate::hud::image(
                assets,
                width,
                height,
                &character_name,
                health,
                max_health,
                mana,
                max_mana,
                level,
                experience,
                &action_bar,
                hover,
                action_context_slot,
                &chat_text,
                chat_focused,
                &chat_history,
                chat_filter,
                double_experience_active,
                heaven_blessing_active,
                burning_active,
                stunned_active,
                cyclone_active,
                superman_active,
                stigma_active,
                shield_active,
                hud_config,
                &skills,
            ),
            Self::StatusBase {
                active_tab,
                weapon_filter,
                close_hovered,
                attribute_details,
                ..
            } => crate::shop::status_base_image(assets, active_tab, weapon_filter, close_hovered, attribute_details),
            Self::StatusDynamic {
                level,
                experience,
                nobility_rank,
                character,
                active_tab,
                mana,
                maximum_mana,
                known_skills,
                equipment,
                weapon_skills,
                weapon_filter,
                skill_page,
                attribute_details,
                ..
            } => crate::shop::status_dynamic_image(
                assets,
                level,
                experience,
                nobility_rank,
                &character,
                active_tab,
                mana,
                maximum_mana,
                &known_skills,
                &equipment,
                &weapon_skills,
                weapon_filter,
                skill_page,
                attribute_details,
            ),
            Self::StatusOverlay {
                equipment,
                hovered_slot,
                ..
            } => crate::shop::status_equipment_overlay_image(assets, &equipment, hovered_slot),
            Self::InventoryBase { .. } => crate::shop::inventory_base_image(assets),
            Self::InventoryOverlay {
                inventory,
                silver,
                cps,
                selected_uid,
                filter,
                hovered_filter,
                close_hovered,
                organize_hovered,
                ..
            } => crate::shop::inventory_overlay_image(
                assets,
                &inventory,
                silver,
                cps,
                selected_uid,
                filter,
                hovered_filter,
                close_hovered,
                organize_hovered,
            ),
            Self::ShopBase { .. } => crate::shop::panel_base_image(assets),
            Self::ShopOverlay { state, .. } => crate::shop::panel_overlay_image(assets, &state),
            Self::WarehouseBase {
                guild_warehouse, ..
            } => crate::shop::warehouse_base_image(assets, guild_warehouse),
            Self::WarehouseOverlay {
                items,
                page,
                selected_uid,
                saved_silver,
                amount_text,
                amount_focused,
                guild_warehouse,
                ..
            } => crate::shop::warehouse_overlay_image(
                assets,
                &items,
                page,
                selected_uid,
                saved_silver,
                &amount_text,
                amount_focused,
                guild_warehouse,
            ),
            Self::QuickPanel {
                panel,
                character,
                action_bar,
                known_skills,
                team_members,
                team_leader_id,
                pending_invite,
                pending_join_request,
                team_join_enabled,
                guild_members,
                pending_guild_invite,
                friends,
                ignores,
                pending_friend_name,
                selected_target_name,
                inventory,
                trade,
                booth,
                own_character_id,
                hud_config,
                bounty,
                store,
                daily,
                highscores,
                nobility,
                guild,
                ..
            } => crate::shop::quick_panel_image(
                assets,
                panel,
                &character,
                &action_bar,
                &known_skills,
                &team_members,
                team_leader_id,
                pending_invite,
                pending_join_request,
                team_join_enabled,
                &guild_members,
                pending_guild_invite,
                &friends,
                &ignores,
                pending_friend_name.as_deref(),
                selected_target_name.as_deref(),
                &inventory,
                &trade,
                &booth,
                own_character_id,
                hud_config,
                bounty.as_ref(),
                store.as_ref(),
                daily.as_ref(),
                highscores.as_ref(),
                nobility.as_ref(),
                guild.as_ref(),
            ),
        };
        image.map_err(|error| error.to_string())
    }
}

pub struct Renderer {
    /// The chat log to draw this frame, and its line menu with the option
    /// under the cursor.
    pub chat_log: Option<crate::chat_log::ChatLogView>,
    pub chat_log_rect: [i32; 4],
    pub chat_menu: Option<(crate::chat_log::LineMenu, Option<usize>)>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    /// What the surface can present with; VSync off picks from these.
    present_modes: Vec<wgpu::PresentMode>,
    layout: wgpu::BindGroupLayout,
    sprites: wgpu::RenderPipeline,
    actor: wgpu::RenderPipeline,
    /// Additive effects: their light is added to what is behind them
    /// (D3D SRCALPHA, ONE), never darkening it.
    glow: wgpu::RenderPipeline,
    depth: wgpu::TextureView,
    vertex_buffer: wgpu::Buffer,
    vertex_buffer_capacity: usize,
    perf_enabled: bool,
    perf_started: Instant,
    perf_frames: u32,
    perf_frame_time: Duration,
    perf_max_frame_time: Duration,
    perf_build_time: Duration,
    perf_max_build_time: Duration,
    perf_upload_time: Duration,
    perf_max_upload_time: Duration,
    perf_acquire_time: Duration,
    perf_max_acquire_time: Duration,
    perf_encode_time: Duration,
    perf_max_encode_time: Duration,
    perf_submit_time: Duration,
    perf_max_submit_time: Duration,
    perf_last_draws: usize,
    perf_last_vertices: usize,
    perf_overlay_updated: Instant,
    max_texture_loads_per_frame: usize,
    /// Textures asked of the loader this frame (`TEXTURE_REQUESTS_PER_FRAME`).
    texture_requests_this_frame: usize,
    texture_loads_this_frame: usize,
    textures: HashMap<String, Texture>,
    missing: HashSet<String>,
    loading: HashSet<String>,
    ready_textures: VecDeque<LoadedTexture>,
    texture_loader: Option<TextureLoader>,
    ui_texture_loader: Option<UiTextureLoader>,
    geometry_loader: Option<GeometryLoader>,
    geometry_loading: HashSet<GeometryKey>,
    static_npc_vertices: HashMap<(u32, i8), Arc<Vec<([f32; 3], [f32; 2])>>>,
    static_remote_pose_vertices: HashMap<(u64, i8, usize), Arc<Vec<([f32; 3], [f32; 2])>>>,
    static_map_cache: Option<StaticMapCache>,
    static_visibility_cache: Option<StaticVisibilityCache>,
    minimap: Option<crate::minimap::Minimap>,
    minimap_map: Option<u32>,
    minimap_ready: Option<(
        u32,
        std::result::Result<Option<crate::minimap::Minimap>, String>,
    )>,
    minimap_upload_index: usize,
    /// The bar's Map checkbox: the corner map is shown.
    pub minimap_shown: bool,
    /// How far Shift Screen has moved the view off the hero.
    pub camera_shift: [f32; 2],
    /// How large the world is drawn (the mouse wheel, `ZOOM_RANGE`): at 2
    /// the screen shows half as much of it, twice as large; the interface
    /// keeps its size.
    pub zoom: f32,
    hud_signature: Option<u64>,
    shop_signature: Option<u64>,
    last_shop_base_key: Option<String>,
    last_shop_overlay_key: Option<String>,
    inventory_signature: Option<u64>,
    last_inventory_key: Option<String>,
    last_inventory_overlay_key: Option<String>,
    warehouse_signature: Option<u64>,
    last_warehouse_base_key: Option<String>,
    last_warehouse_overlay_key: Option<String>,
    status_signature: Option<u64>,
    last_status_base_key: Option<String>,
    last_status_dynamic_key: Option<String>,
    last_status_overlay_key: Option<String>,
    feedback_signature: Option<u64>,
    /// When the shown feedback message last changed.
    feedback_since: f32,
    /// Frames drawn, and the last frame each transient texture was drawn in.
    frame_number: u64,
    transient_used: HashMap<String, u64>,
    quick_panel_signature: Option<u64>,
    /// The quick panel's last texture that was ready, with its panel: drawn
    /// while a new one is being made, so the window never blinks.
    quick_panel_shown: Option<(crate::shop::QuickPanel, String)>,
    /// Where the bounty line under the minimap was drawn, for clicks.
    bounty_tracker_rect: Option<[f32; 4]>,
    /// The team's Rift run and when the server told of it; the tracker
    /// goes under the bounty's.
    pub rift: Option<(gravak_protocol::RiftState, Instant)>,
    /// What the hero has discovered of a crypt.
    reveal: Option<crate::reveal::Reveal>,
    /// The crypt and how much of it the minimap shows (`Reveal::generation`).
    minimap_discovered: Option<(u32, u32)>,
    /// Where the Store icon was drawn, for clicks.
    store_icon_rect: Option<[f32; 4]>,
    ranking_icon_rect: Option<[f32; 4]>,
    helper_icon_rect: Option<[f32; 4]>,
    /// Whether the helper is on: its button's light.
    pub helper_on: bool,
    /// The Helper button's hint ("Helper (J)").
    pub helper_tip: String,
    /// The Nobility Rank button beside the map plate.
    nobility_icon_rect: Option<[f32; 4]>,
    /// Where the revive buttons were drawn (and whether each revives
    /// here), for clicks.
    revive_rects: Vec<([f32; 4], bool)>,
    /// Souls of slain monsters on their way to whoever slew them.
    souls: Vec<Soul>,
    /// Arrows in flight, and the flying models by path (`None` when one
    /// cannot be read).
    arrows: Vec<Arrow>,
    flying_models: HashMap<String, Option<std::rc::Rc<C3Document>>>,
    /// Effects played once on an actor: (actor, effect, started).
    role_effects: Vec<(u32, String, f32)>,
    level_up_layers: Option<Vec<LevelUpLayer>>,
    /// Original 3D effects by name; `None` when the effect is unavailable.
    effects: HashMap<String, Option<std::rc::Rc<LoadedEffect>>>,
    /// Where each actor with a status effect stood lately (time, world
    /// pixel): the ribbons of shape effects stream along it.
    trail_history: HashMap<u32, std::collections::VecDeque<(f32, [f32; 2])>>,
    /// How large world effects are drawn (0.75); the XP fire is drawn 1:1.
    effect_scale: f32,
    /// The trail of the actor whose status effects are being drawn, in
    /// screen pixels, newest first, and the time it is drawn at.
    current_trail: (f32, Vec<(f32, [f32; 2])>),
    /// The facing an effect is drawn turned by (a weapon's swing turns
    /// with its wielder); `None` for effects that face the camera.
    effect_yaw: Option<f32>,
    /// Where an effect bound to a weapon hangs (the weapon's posed bone);
    /// `None` for effects placed at the actor's feet.
    effect_attachment: Option<Matrix4>,
    /// The tint an effect is drawn recoloured with (a rarity's glow).
    effect_tint: Option<&'static str>,
    /// The Shortcuts window, while open (set each frame by the app).
    pub shortcuts: Option<crate::keybinds::ShortcutsView>,
    pub helper: Option<crate::helper::HelperView>,
    /// "While you were away", while it is open (set each frame by the app).
    pub idle_report: Option<crate::idle_report::IdleReportView>,
    pub travel_cube: Option<crate::travel_cube::TravelCubeView>,
    /// Weapon swings playing (`Action3DEffect.ini` `Flash…`): whose (`None`
    /// the hero), the effect and when it began.
    weapon_flashes: Vec<(Option<u32>, String, f32)>,
    /// The last swing seen of the hero (its action instance) and of others
    /// (their animation start), so each starts its flash once.
    last_hero_swing: u64,
    last_remote_swing: HashMap<u32, f32>,
    /// `Action3DEffect.ini`: action, weapon type and item
    /// (`<action>.<type>.<item % 1000>`) to its effect: 401 a blow's flash,
    /// 999 what a Super weapon always shows.
    swing_effects: Option<HashMap<String, String>>,
    last_hud_key: Option<String>,
    /// In the browser, `web_files::misses` when the last texture was made:
    /// a picture made while a file was still on its way holds a stand-in
    /// (a square with the item's number) and is not kept.
    #[cfg(target_arch = "wasm32")]
    upload_misses: u64,
}
impl Renderer {
    pub async fn new(window: Arc<Window>) -> Result<Self> {
        #[cfg(not(target_arch = "wasm32"))]
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        // In the browser, WebGL2 where WebGPU is not offered or has no
        // adapter (docs/web-spike.md).
        #[cfg(target_arch = "wasm32")]
        let instance =
            wgpu::util::new_instance_with_webgpu_detection(wgpu::InstanceDescriptor::new_without_display_handle())
                .await;
        let surface = instance.create_surface(window.clone())?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await?;
        let descriptor = wgpu::DeviceDescriptor::default();
        // WebGL2 has none of WebGPU's compute limits: ask for what it has.
        #[cfg(target_arch = "wasm32")]
        let descriptor = if adapter.get_info().backend == wgpu::Backend::Gl {
            wgpu::DeviceDescriptor {
                required_limits: wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
                ..descriptor
            }
        } else {
            descriptor
        };
        let (device, queue) = adapter.request_device(&descriptor).await?;
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or("No surface format")?;
        // Blend as the original's Direct3D did, on the stored values (not in
        // linear light): its additive glows add up to their full brightness.
        let plain = config.format.remove_srgb_suffix();
        if surface.get_capabilities(&adapter).formats.contains(&plain) {
            config.format = plain;
        }
        let requested_present_mode = std::env::var("GRAVAK_PRESENT_MODE")
            .unwrap_or_default()
            .to_ascii_lowercase();
        let present_modes = surface.get_capabilities(&adapter).present_modes;
        config.present_mode = match requested_present_mode.as_str() {
            "immediate" if present_modes.contains(&wgpu::PresentMode::Immediate) => {
                wgpu::PresentMode::Immediate
            }
            "mailbox" if present_modes.contains(&wgpu::PresentMode::Mailbox) => {
                wgpu::PresentMode::Mailbox
            }
            "fifo" if present_modes.contains(&wgpu::PresentMode::Fifo) => wgpu::PresentMode::Fifo,
            _ => wgpu::PresentMode::AutoVsync,
        };
        if !requested_present_mode.is_empty() {
            eprintln!(
                "PRESENT requested={} selected={:?}",
                requested_present_mode, config.present_mode
            );
        }
        surface.configure(&device, &config);
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(include_str!("textured.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        // Light added onto the scene, as the original's additive effects.
        let additive = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Zero,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
        };
        let pipeline = |actor, blend: wgpu::BlendState| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vertex_main"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: 24,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x2,2=>Float32],
                    })],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fragment_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: config.format,
                        blend: Some(blend),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(actor),
                    depth_compare: Some(if actor {
                        wgpu::CompareFunction::LessEqual
                    } else {
                        wgpu::CompareFunction::Always
                    }),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let sprites = pipeline(false, wgpu::BlendState::ALPHA_BLENDING);
        let actor = pipeline(true, wgpu::BlendState::ALPHA_BLENDING);
        let glow = pipeline(false, additive);
        let depth = depth(&device, &config);
        let vertex_buffer_capacity = 64 * 1024;
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("renderer dynamic vertex buffer"),
            size: vertex_buffer_capacity as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Ok(Self {
            surface,
            device,
            queue,
            config,
            present_modes,
            layout,
            sprites,
            actor,
            glow,
            depth,
            vertex_buffer,
            vertex_buffer_capacity,
            perf_enabled: std::env::var_os("GRAVAK_PERF").is_some(),
            perf_started: Instant::now(),
            perf_frames: 0,
            perf_frame_time: Duration::ZERO,
            perf_max_frame_time: Duration::ZERO,
            perf_build_time: Duration::ZERO,
            perf_max_build_time: Duration::ZERO,
            perf_upload_time: Duration::ZERO,
            perf_max_upload_time: Duration::ZERO,
            perf_acquire_time: Duration::ZERO,
            perf_max_acquire_time: Duration::ZERO,
            perf_encode_time: Duration::ZERO,
            perf_max_encode_time: Duration::ZERO,
            perf_submit_time: Duration::ZERO,
            perf_max_submit_time: Duration::ZERO,
            perf_last_draws: 0,
            perf_last_vertices: 0,
            perf_overlay_updated: Instant::now(),
            max_texture_loads_per_frame: std::env::var("GRAVAK_ASSET_LOADS_PER_FRAME")
                .ok()
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(1)
                .clamp(1, 64),
            texture_loads_this_frame: 0,
            texture_requests_this_frame: 0,
            textures: HashMap::new(),
            missing: HashSet::new(),
            loading: HashSet::new(),
            ready_textures: VecDeque::new(),
            texture_loader: None,
            ui_texture_loader: None,
            geometry_loader: None,
            geometry_loading: HashSet::new(),
            static_npc_vertices: HashMap::new(),
            static_remote_pose_vertices: HashMap::new(),
            static_map_cache: None,
            static_visibility_cache: None,
            minimap: None,
            minimap_map: None,
            minimap_ready: None,
            minimap_upload_index: 0,
            minimap_shown: true,
            camera_shift: [0.0, 0.0],
            zoom: 1.0,
            hud_signature: None,
            shop_signature: None,
            last_shop_base_key: None,
            last_shop_overlay_key: None,
            inventory_signature: None,
            last_inventory_key: None,
            last_inventory_overlay_key: None,
            warehouse_signature: None,
            last_warehouse_base_key: None,
            last_warehouse_overlay_key: None,
            status_signature: None,
            last_status_base_key: None,
            last_status_dynamic_key: None,
            last_status_overlay_key: None,
            feedback_signature: None,
            feedback_since: 0.0,
            frame_number: 0,
            transient_used: HashMap::new(),
            quick_panel_signature: None,
            quick_panel_shown: None,
            bounty_tracker_rect: None,
            rift: None,
            reveal: None,
            minimap_discovered: None,
            store_icon_rect: None,
            ranking_icon_rect: None,
            helper_icon_rect: None,
            helper_on: false,
            helper_tip: String::new(),
            nobility_icon_rect: None,
            revive_rects: Vec::new(),
            souls: Vec::new(),
            arrows: Vec::new(),
            flying_models: HashMap::new(),
            role_effects: Vec::new(),
            level_up_layers: None,
            effects: HashMap::new(),
            trail_history: HashMap::new(),
            effect_scale: 0.75,
            current_trail: (0.0, Vec::new()),
            effect_yaw: None,
            effect_attachment: None,
            effect_tint: None,
            shortcuts: None,
            helper: None,
            idle_report: None,
            travel_cube: None,
            weapon_flashes: Vec::new(),
            last_hero_swing: 0,
            last_remote_swing: HashMap::new(),
            swing_effects: None,
            last_hud_key: None,
            #[cfg(target_arch = "wasm32")]
            upload_misses: 0,
            chat_log: None,
            chat_log_rect: [0; 4],
            chat_menu: None,
        })
    }

    /// The swing effect a weapon's blow plays (`Action3DEffect.ini`, the
    /// attack motion 401: `999.401.<type>.<item % 1000>`).
    fn swing_effect(&mut self, assets: &mut Assets, weapon: u32) -> Option<String> {
        self.weapon_effect(assets, "401", weapon)
    }

    /// A weapon's effect in `action` (`Action3DEffect.ini`, any body).
    fn weapon_effect(&mut self, assets: &mut Assets, action: &str, weapon: u32) -> Option<String> {
        if weapon == 0 {
            return None;
        }
        let catalog = self.swing_effects.get_or_insert_with(|| {
            let text = assets.read("ini/Action3DEffect.ini").map(|bytes| String::from_utf8_lossy(&bytes).into_owned()).unwrap_or_default();
            text.lines()
                .filter_map(|line| {
                    let (key, effect) = line.trim().split_once('=')?;
                    let mut parts = key.split('.');
                    let (body, action, kind, item) = (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
                    let effect = effect.trim();
                    (body == "999" && effect != "none").then(|| (format!("{action}.{kind}.{item}"), effect.to_owned()))
                })
                .collect()
        });
        catalog.get(&format!("{action}.{}.{:03}", weapon / 1_000, weapon % 1_000)).cloned()
    }

    /// Draws the swings of `who` at its feet, turned by its facing; the
    /// finished ones go.
    #[allow(clippy::too_many_arguments)]
    fn push_weapon_flashes(
        &mut self,
        assets: &mut Assets,
        who: Option<u32>,
        origin: [f32; 2],
        facing: f32,
        time: f32,
        sort_depth: f32,
        viewport: [f32; 2],
        draws: &mut Vec<Draw>,
    ) {
        let flashes = std::mem::take(&mut self.weapon_flashes);
        let mut kept = Vec::with_capacity(flashes.len());
        for (owner, name, started) in flashes {
            if owner != who {
                kept.push((owner, name, started));
                continue;
            }
            self.effect_yaw = Some(facing);
            let playing = self.push_effect(assets, &name, origin, time - started, sort_depth, viewport, draws);
            self.effect_yaw = None;
            // A blow's flash lasts its effect; never more than two seconds.
            if playing && time - started < 2.0 {
                kept.push((owner, name, started));
            }
        }
        // Flashes of others drawn later this frame stay.
        kept.extend(std::mem::take(&mut self.weapon_flashes));
        self.weapon_flashes = kept;
    }

    /// The original's lights under item icons (`shop::item_light`), each
    /// over its slot of a window at `origin`.
    fn push_item_lights(
        &mut self,
        assets: &Assets,
        lights: &[(&'static str, crate::ui::UiRect)],
        origin: [f32; 2],
        time: f32,
        viewport: [f32; 2],
        draws: &mut Vec<Draw>,
    ) {
        for (light, rect) in lights {
            let frame = crate::shop::item_light_frame(light, time);
            self.texture(assets, &frame);
            if self.textures.contains_key(&frame) {
                draws.push(Draw {
                    texture: frame,
                    vertices: quad(
                        [origin[0] + rect.x as f32, origin[1] + rect.y as f32],
                        [rect.width as f32, rect.height as f32],
                        viewport,
                    ),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
        }
    }

    /// What Super weapons always show (`Action3DEffect.ini` 999.999),
    /// bound to each weapon where it is held.
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_arguments)]
    fn push_weapon_auras(
        &mut self,
        assets: &mut Assets,
        weapons: Vec<(u32, Matrix4, bool)>,
        rarity: [u8; 2],
        origin: [f32; 2],
        facing: f32,
        time: f32,
        sort_depth: f32,
        viewport: [f32; 2],
        draws: &mut Vec<Draw>,
    ) {
        for (weapon, matrix, left) in weapons {
            self.effect_yaw = Some(facing);
            self.effect_attachment = Some(matrix);
            // Legendary and Mythic glow gold or crimson: a Super weapon's own
            // glow turns that colour (never a second glow over it); another
            // weapon takes its kind's top Super glow in that colour.
            // An accessory's aura keeps the colour it was given.
            let tint = crate::shop::accessory_aura_tint(assets, weapon)
                .or_else(|| crate::magic::rarity_tint(rarity[usize::from(left)]));
            let glow = match self.weapon_effect(assets, "999", weapon) {
                Some(own) => Some(own),
                None if tint.is_some() => self.kind_glow(assets, weapon),
                None => None,
            };
            if let Some(name) = glow {
                self.effect_tint = tint;
                self.push_looping_effect(assets, &name, origin, time, sort_depth, viewport, draws);
                self.effect_tint = None;
            }
            self.effect_attachment = None;
            self.effect_yaw = None;
            let sparkles = crate::shop::accessory_sparkles(assets, weapon);
            if !sparkles.is_empty() {
                self.push_sparkles(sparkles, matrix, tint, origin, facing, time, sort_depth, viewport, draws);
            }
        }
    }

    /// A soul `elapsed` seconds after its monster fell at `from` (on screen):
    /// first its head, a star of its light, rising over the body; then
    /// flying along `path` (0 where it rose to, 1 its slayer), faster and
    /// faster, its trail a ribbon over the last of its way, broad and bright
    /// at its head and thinning to nothing.
    fn push_soul(
        &mut self,
        path: impl Fn(f32) -> [f32; 2],
        from: [f32; 2],
        elapsed: f32,
        sort_depth: f32,
        viewport: [f32; 2],
        draws: &mut Vec<Draw>,
    ) {
        let trail = format!("@soul-trail{}", crate::magic::ADDITIVE_SUFFIX);
        if !self.textures.contains_key(&trail) {
            self.upload(&trail, soul_trail_image(SOUL_TINT));
        }
        let head = format!("@soul-head{}", crate::magic::ADDITIVE_SUFFIX);
        if !self.textures.contains_key(&head) {
            self.upload(&head, sparkle_image(SOUL_TINT));
        }
        // It comes in and goes out over a tenth of a second or so.
        let fade = (elapsed / 0.1).min(1.0) * ((SOUL_RISE_SECONDS + SOUL_FLIGHT_SECONDS - elapsed) / 0.15).clamp(0.0, 1.0);
        let flight = ((elapsed - SOUL_RISE_SECONDS) / SOUL_FLIGHT_SECONDS).clamp(0.0, 1.0);
        let head_at = if flight <= 0.0 {
            let rise = (elapsed / SOUL_RISE_SECONDS).clamp(0.0, 1.0);
            [from[0], from[1] - 40.0 - rise * (2.0 - rise) * 30.0]
        } else {
            path(flight * flight)
        };
        let size = 26.0 + 4.0 * (elapsed * 9.0).sin();
        let vertices = quad_with_alpha([head_at[0] - size / 2.0, head_at[1] - size / 2.0], [size, size], viewport, fade);
        draws.push(Draw { texture: head, vertices, depth_test: false, sort_depth });
        if flight <= 0.0 {
            return;
        }
        const STEPS: usize = 28;
        let way = flight * flight;
        let start = (way - SOUL_TRAIL).max(0.0);
        let points: Vec<[f32; 2]> = (0..=STEPS).map(|k| path(start + (way - start) * k as f32 / STEPS as f32)).collect();
        // Each point's two edges and light: 0 at the tail, full at the head.
        let edge = |k: usize| -> ([f32; 2], [f32; 2], f32) {
            let (a, b) = (points[k.saturating_sub(1)], points[(k + 1).min(STEPS)]);
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let length = dx.hypot(dy).max(0.001);
            let along = k as f32 / STEPS as f32;
            let half = 1.0 + 7.0 * along;
            let normal = [-dy / length * half, dx / length * half];
            let p = points[k];
            ([p[0] + normal[0], p[1] + normal[1]], [p[0] - normal[0], p[1] - normal[1]], along * along * fade)
        };
        let corner = |p: [f32; 2], v: f32, alpha: f32| Vertex { position: clip(p, 0.5, viewport), uv: [0.5, v], alpha };
        let mut vertices = Vec::with_capacity(STEPS * 6);
        for k in 0..STEPS {
            let ((l0, r0, a0), (l1, r1, a1)) = (edge(k), edge(k + 1));
            vertices.extend([
                corner(l0, 0.0, a0),
                corner(r0, 1.0, a0),
                corner(r1, 1.0, a1),
                corner(l0, 0.0, a0),
                corner(r1, 1.0, a1),
                corner(l1, 0.0, a1),
            ]);
        }
        draws.push(Draw { texture: trail, vertices, depth_test: false, sort_depth });
    }

    /// An accessory's sparkles: little stars over the weapon (`points`, in
    /// its rest pose, carried by the hand's `matrix` as its aura is), each
    /// twinkling in its own time, white at the heart and `tint` about it.
    #[allow(clippy::too_many_arguments)]
    fn push_sparkles(
        &mut self,
        points: &[[f32; 3]],
        matrix: Matrix4,
        tint: Option<&'static str>,
        origin: [f32; 2],
        facing: f32,
        time: f32,
        sort_depth: f32,
        viewport: [f32; 2],
        draws: &mut Vec<Draw>,
    ) {
        let tint = tint.unwrap_or("ice");
        let key = format!("@sparkle-{tint}{}", crate::magic::ADDITIVE_SUFFIX);
        if !self.textures.contains_key(&key) {
            self.upload(&key, sparkle_image(crate::magic::tint(tint).unwrap_or([0.5, 0.8, 1.0])));
        }
        let (sin, cos) = (-std::f32::consts::FRAC_PI_4).sin_cos();
        let scale = self.effect_scale;
        for (index, point) in points.iter().enumerate() {
            // Each its own pace and moment, from its place in the list.
            let seed = (index as f32 * 12.9898).sin() * 43_758.547;
            let seed = seed - seed.floor();
            let light = (time * (2.0 + seed * 2.5) + seed * std::f32::consts::TAU).sin().max(0.0).powi(2);
            if light < 0.02 {
                continue;
            }
            let p = turned(matrix.transform_point(sandbox_data::c3::Vector3 { x: point[0], y: point[1], z: point[2] }), Some(facing));
            let at = [origin[0] + p.x * scale, origin[1] + (p.y * sin + p.z * cos) * scale];
            let size = 6.0 + 9.0 * light;
            let vertices = quad_with_alpha([at[0] - size / 2.0, at[1] - size / 2.0], [size, size], viewport, light);
            draws.push(Draw { texture: key.clone(), vertices, depth_test: false, sort_depth });
        }
    }

    /// The Super glow of the highest weapon of `weapon`'s kind (a level 130
    /// blade's for every blade), shaped for that kind of weapon.
    fn kind_glow(&mut self, assets: &mut Assets, weapon: u32) -> Option<String> {
        // The catalog loads with the first lookup.
        let _ = self.weapon_effect(assets, "999", weapon);
        let prefix = format!("999.{}.", weapon / 1_000);
        let catalog = self.swing_effects.as_ref()?;
        catalog
            .iter()
            .filter_map(|(key, name)| Some((key.strip_prefix(&prefix)?.parse::<u32>().ok()?, name)))
            .max_by_key(|(item, _)| *item)
            .map(|(_, name)| name.clone())
    }

    /// Plays an effect over and over (status effects): the time wraps at
    /// the effect's length.
    fn push_looping_effect(
        &mut self,
        assets: &mut Assets,
        name: &str,
        origin: [f32; 2],
        time: f32,
        sort_depth: f32,
        viewport: [f32; 2],
        draws: &mut Vec<Draw>,
    ) {
        let length = self
            .effects
            .get(name)
            .cloned()
            .flatten()
            .map(|effect| effect.frames * effect.frame_interval / 1000.0)
            .filter(|length| *length > 0.0)
            .unwrap_or(1.0);
        self.push_effect(assets, name, origin, time % length, sort_depth, viewport, draws);
    }

    /// Plays an original 3D effect (`3DEffect.ini`) at a screen point.
    /// Returns false once the effect has finished.
    fn push_effect(
        &mut self,
        assets: &mut Assets,
        name: &str,
        origin: [f32; 2],
        elapsed: f32,
        sort_depth: f32,
        viewport: [f32; 2],
        draws: &mut Vec<Draw>,
    ) -> bool {
        self.push_effect_shaped(assets, name, origin, elapsed, sort_depth, viewport, draws, None)
    }

    /// A flat 2D effect (the original's status flags, 25 units square)
    /// drawn `size` pixels wide over the interface, looping.
    fn push_ui_effect(
        &mut self,
        assets: &mut Assets,
        name: &str,
        centre: [f32; 2],
        time: f32,
        size: f32,
        viewport: [f32; 2],
        draws: &mut Vec<Draw>,
    ) {
        let length = self
            .effects
            .get(name)
            .cloned()
            .flatten()
            .map(|effect| effect.frames * effect.frame_interval / 1000.0)
            .filter(|length| *length > 0.0)
            .unwrap_or(1.0);
        self.push_effect_shaped(assets, name, centre, time % length, f32::INFINITY, viewport, draws, Some(size));
    }

    /// `flat`: draw the effect face on, `flat` pixels for its 25 units,
    /// instead of seen from the world camera.
    #[allow(clippy::too_many_arguments)]
    fn push_effect_shaped(
        &mut self,
        assets: &mut Assets,
        name: &str,
        origin: [f32; 2],
        elapsed: f32,
        sort_depth: f32,
        viewport: [f32; 2],
        draws: &mut Vec<Draw>,
        flat: Option<f32>,
    ) -> bool {
        if !self.effects.contains_key(name) {
            let loaded = crate::magic::effect(assets, name).and_then(|definition| {
                let layers = definition
                    .layers
                    .iter()
                    .filter_map(|layer| {
                        Some(LevelUpLayer {
                            document: C3Document::parse(&assets.read(&layer.mesh).ok()?).ok()?,
                            texture: layer.texture_key(),
                        })
                    })
                    .collect::<Vec<_>>();
                let frames = layers
                    .iter()
                    .flat_map(|layer| {
                        layer
                            .document
                            .motions
                            .iter()
                            .map(|motion| motion.frame_count as f32)
                            .chain(layer.document.meshes.iter().flat_map(|mesh| {
                                mesh.transparency_keys
                                    .last()
                                    .map(|key| key.frame as f32)
                                    .into_iter()
                                    .chain(mesh.draw_keys.last().map(|key| key.frame as f32))
                            }))
                            .chain(layer.document.shapes.iter().map(|shape| shape.frames.len() as f32))
                            .chain(layer.document.particles.iter().map(|cloud| cloud.frames.len() as f32))
                    })
                    .fold(1.0_f32, f32::max);
                (!layers.is_empty()).then(|| {
                    std::rc::Rc::new(LoadedEffect {
                        layers,
                        frame_interval: definition.frame_interval,
                        frames,
                    })
                })
            });
            self.effects.insert(name.to_owned(), loaded);
        }
        let Some(effect) = self.effects.get(name).cloned().flatten() else {
            return false;
        };
        let frame = elapsed.max(0.0) * 1000.0 / effect.frame_interval;
        if frame >= effect.frames {
            return false;
        }
        let tint = self.effect_tint;
        let key_of = |texture: &str| match tint {
            Some(tint) => crate::magic::tinted_key(texture, tint),
            None => texture.to_owned(),
        };
        for layer in &effect.layers {
            self.texture(assets, &key_of(&layer.texture));
        }
        let (sin, cos) = (-std::f32::consts::FRAC_PI_4).sin_cos();
        let scale = self.effect_scale;
        let yaw = self.effect_yaw;
        let attachment = self.effect_attachment;
        // A flat effect (a status flag) is moved by its own animation far
        // from its origin: centre it on its slot by the box it fills.
        let centre = flat.map(|_| {
            let (mut low, mut high) = ([f32::MAX; 2], [f32::MIN; 2]);
            for layer in &effect.layers {
                for (mesh_index, mesh) in layer.document.meshes.iter().enumerate() {
                    let Some(motion) = layer.document.motions.get(mesh_index).or_else(|| layer.document.motions.first())
                    else {
                        continue;
                    };
                    for vertex in &mesh.vertices {
                        let p = mesh.transformed_vertex_position(vertex, motion, Matrix4::IDENTITY, 0.0);
                        let point = [p.x, p.y * sin + p.z * cos];
                        for axis in 0..2 {
                            low[axis] = low[axis].min(point[axis]);
                            high[axis] = high[axis].max(point[axis]);
                        }
                    }
                }
            }
            let span = (high[0] - low[0]).max(high[1] - low[1]).max(1.0);
            ([(low[0] + high[0]) / 2.0, (low[1] + high[1]) / 2.0], span)
        });
        for layer in &effect.layers {
            let texture = key_of(&layer.texture);
            if !self.textures.contains_key(&texture) {
                continue;
            }
            for (mesh_index, mesh) in layer.document.meshes.iter().enumerate() {
                if !draw_key_visible(&mesh.draw_keys, frame) {
                    continue;
                }
                let Some(motion) = layer
                    .document
                    .motions
                    .get(mesh_index)
                    .or_else(|| layer.document.motions.first())
                else {
                    continue;
                };
                let alpha = transparency_at(&mesh.transparency_keys, frame);
                let cell = sheet_cell(mesh, frame);
                let vertices: Vec<Vertex> = mesh
                    .vertices
                    .iter()
                    .map(|vertex| {
                        let p = mesh.transformed_vertex_position(
                            vertex,
                            motion,
                            attachment.unwrap_or(Matrix4::IDENTITY),
                            frame,
                        );
                        let p = turned(p, yaw);
                        let (screen, depth) = match (flat, centre) {
                            (Some(size), Some((middle, span))) => {
                                let unit = size / span;
                                let screen_y = p.y * sin + p.z * cos;
                                (
                                    [origin[0] + (p.x - middle[0]) * unit, origin[1] + (screen_y - middle[1]) * unit],
                                    0.5,
                                )
                            },
                            _ => {
                                let screen_y = (p.y * sin + p.z * cos) * scale;
                                let depth = (p.y * cos - p.z * sin) * scale;
                                ([origin[0] + p.x * scale, origin[1] + screen_y], (0.5 + depth * 0.0005).clamp(0.001, 0.999))
                            },
                        };
                        Vertex {
                            position: clip(screen, depth, viewport),
                            uv: [vertex.texture_coordinate.x + cell[0], vertex.texture_coordinate.y + cell[1]],
                            alpha,
                        }
                    })
                    .collect();
                draws.push(Draw {
                    texture: texture.clone(),
                    vertices: mesh
                        .opaque_indices
                        .iter()
                        .chain(&mesh.alpha_indices)
                        .map(|index| vertices[*index as usize])
                        .collect(),
                    depth_test: false,
                    sort_depth,
                });
            }
            // Shapes sweep a ribbon through their last places, fading
            // towards the oldest.
            for shape in &layer.document.shapes {
                let segments = shape.segments.max(2) as usize;
                // Each step back is a frame back: on the actor's trail
                // where they stood then, or here when they stood still.
                let step_origin = |step: usize| {
                    let (now, trail) = &self.current_trail;
                    let when = now - step as f32 * effect.frame_interval / 1000.0;
                    trail_point(trail, when).unwrap_or(origin)
                };
                let project = |p: sandbox_data::c3::Vector3, origin: [f32; 2]| {
                    let p = attachment.map_or(p, |matrix| matrix.transform_point(p));
                    let p = turned(p, yaw);
                    let screen_y = (p.y * sin + p.z * cos) * scale;
                    let depth = (p.y * cos - p.z * sin) * scale;
                    ([origin[0] + p.x * scale, origin[1] + screen_y], (0.5 + depth * 0.0005).clamp(0.001, 0.999))
                };
                let origins: Vec<[f32; 2]> = (0..segments).map(step_origin).collect();
                let mut vertices = Vec::new();
                for line in &shape.lines {
                    if line.len() < 2 {
                        continue;
                    }
                    let places: Vec<Vec<sandbox_data::c3::Vector3>> =
                        (0..segments).map(|step| shape.line_at(line, frame - step as f32)).collect();
                    let last = (line.len() - 1) as f32;
                    for step in 0..segments - 1 {
                        let fade = |step: usize| 1.0 - step as f32 / (segments - 1) as f32;
                        for point in 0..line.len() - 1 {
                            let corner = |step: usize, point: usize| {
                                let (screen, depth) = project(places[step][point], origins[step]);
                                Vertex {
                                    position: clip(screen, depth, viewport),
                                    uv: [step as f32 / (segments - 1) as f32, point as f32 / last],
                                    alpha: fade(step),
                                }
                            };
                            let (a, b) = (corner(step, point), corner(step, point + 1));
                            let (c, d) = (corner(step + 1, point + 1), corner(step + 1, point));
                            vertices.extend([a, b, c, a, c, d]);
                        }
                    }
                }
                if !vertices.is_empty() {
                    draws.push(Draw {
                        texture: texture.clone(),
                        vertices,
                        depth_test: false,
                        sort_depth,
                    });
                }
            }
            // Particles: each a square facing the camera, its cell of the
            // sheet chosen by its age (`PTCL`).
            for cloud in &layer.document.particles {
                if flat.is_some() || cloud.frames.is_empty() {
                    continue;
                }
                let sprites = &cloud.frames[frame as usize % cloud.frames.len()];
                let row = cloud.row as f32;
                let cells = cloud.row * cloud.row;
                let mut vertices = Vec::with_capacity(sprites.positions.len() * 6);
                for ((position, age), size) in sprites.positions.iter().zip(&sprites.ages).zip(&sprites.sizes) {
                    let p = sprites.matrix.transform_point(*position);
                    let p = attachment.map_or(p, |matrix| matrix.transform_point(p));
                    let p = turned(p, yaw);
                    let centre = [origin[0] + p.x * scale, origin[1] + (p.y * sin + p.z * cos) * scale];
                    let depth = (0.5 + (p.y * cos - p.z * sin) * scale * 0.0005).clamp(0.001, 0.999);
                    // Each sprite spans its size either side of its place
                    // (C3_CORE_DLL `Ptcl_Draw`): twice its size across.
                    let half = size * scale;
                    let cell = ((age.clamp(0.0, 1.0) * cells as f32) as u32).min(cells - 1);
                    let (u, v) = ((cell % cloud.row) as f32 / row, (cell / cloud.row) as f32 / row);
                    let corner = |dx: f32, dy: f32| Vertex {
                        position: clip([centre[0] + dx * half, centre[1] + dy * half], depth, viewport),
                        uv: [u + (dx + 1.0) / 2.0 / row, v + (dy + 1.0) / 2.0 / row],
                        alpha: 1.0,
                    };
                    let (a, b, c, d) = (corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0));
                    vertices.extend([a, b, c, a, c, d]);
                }
                if !vertices.is_empty() {
                    draws.push(Draw {
                        texture: texture.clone(),
                        vertices,
                        depth_test: false,
                        sort_depth,
                    });
                }
            }
        }
        true
    }

    fn push_status_character(
        &mut self,
        assets: &mut Assets,
        character: &Character,
        panel: [f32; 2],
        viewport: [f32; 2],
        _time: f32,
        draws: &mut Vec<Draw>,
    ) {
        self.push_character_preview(
            assets,
            character,
            [panel[0] + 128.0, panel[1] + 82.0],
            [80.0, 230.0],
            f32::INFINITY,
            viewport,
            draws,
        );
    }

    /// Draws `character` standing, its body `size` ([max width, height])
    /// pixels and centred under `top_center`, like the status paper doll.
    fn push_character_preview(
        &mut self,
        assets: &mut Assets,
        character: &Character,
        top_center: [f32; 2],
        size: [f32; 2],
        sort_depth: f32,
        viewport: [f32; 2],
        draws: &mut Vec<Draw>,
    ) {
        let facing = -std::f32::consts::FRAC_PI_4;
        let facing_sector = (facing / std::f32::consts::FRAC_PI_4).round() as i8;
        let signature = appearance_signature(character);
        let mut body_bounds = [[f32::INFINITY, f32::INFINITY], [f32::NEG_INFINITY; 2]];
        let mut poses = Vec::with_capacity(character.parts.len());
        for (part_index, _part) in character.parts.iter().enumerate() {
            let Some(pose) = self.queue_character_idle_geometry(
                character,
                signature,
                part_index,
                facing_sector,
                facing,
            ) else {
                return;
            };
            for (p, _) in pose.iter() {
                // Part 0 is the body and part 1 is hair. Weapons are
                // appended after those parts. Use only the body silhouette
                // as the anchor so equipping a large weapon cannot recenter
                // or rescale the character itself.
                if part_index < 2 {
                    body_bounds[0][0] = body_bounds[0][0].min(p[0]);
                    body_bounds[0][1] = body_bounds[0][1].min(p[1]);
                    body_bounds[1][0] = body_bounds[1][0].max(p[0]);
                    body_bounds[1][1] = body_bounds[1][1].max(p[1]);
                }
            }
            poses.push(pose);
        }
        let width = body_bounds[1][0] - body_bounds[0][0];
        let height = body_bounds[1][1] - body_bounds[0][1];
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            return;
        }
        // Keep the live C3 preview inside the central character area.  The
        // equipment frames are composited again after the model, but this
        // padding also prevents oversized weapons from dominating the panel.
        // Keep the complete equipped model inside the safe lane between the
        // left and right equipment slots. Large weapons must not push the
        // paper doll underneath the slot frames.
        // Height drives the scale, as in the original paper doll; the width
        // cap only matters for unusually wide bodies. Scaling by width made
        // slim bodies blow up past the panel.
        let scale = (size[1] / height).min(size[0] / width);
        let [center_x, top] = top_center;
        for (part, pose) in character.parts.iter().zip(poses) {
            self.texture(assets, &part.texture);
            let triangles = part
                .mesh
                .opaque_indices
                .iter()
                .chain(&part.mesh.alpha_indices)
                .map(|i| {
                    let (p, uv) = pose[*i as usize];
                    let screen = [
                        center_x + (p[0] - (body_bounds[0][0] + body_bounds[1][0]) * 0.5) * scale,
                        top + (p[1] - body_bounds[0][1]) * scale,
                    ];
                    // Nearer than any world actor (0.5 +- 0.5), so one
                    // standing behind the panel cannot cut into the model.
                    Vertex {
                        position: clip(screen, (0.05 + p[2] * 0.00005).clamp(0.001, 0.099), viewport),
                        uv,
                        alpha: 1.0,
                    }
                })
                .collect();
            draws.push(Draw {
                texture: part.texture.clone(),
                vertices: triangles,
                depth_test: true,
                sort_depth,
            });
        }
    }

    fn cached_static_npc_vertices(
        &mut self,
        model: &crate::character::NpcModel,
        facing: f32,
    ) -> Option<Arc<Vec<([f32; 3], [f32; 2])>>> {
        self.queue_npc_geometry(model, facing)
    }

    fn ensure_static_map_cache(&mut self, map: &Map) {
        if self
            .static_map_cache
            .as_ref()
            .is_some_and(|cache| cache.map_id == map.id && cache.sprite_count == map.sprites.len())
        {
            return;
        }

        let mut static_sprites = Vec::new();
        let mut dynamic_indices = Vec::new();
        for (index, sprite) in map.sprites.iter().enumerate() {
            if sprite.frames.len() != 1 {
                dynamic_indices.push(index);
                continue;
            }
            static_sprites.push(StaticMapSprite {
                texture: sprite.frames[0].clone(),
                origin: sprite.origin,
                depth: sprite.depth,
                cell: sprite.cell,
            });
        }
        let mut grid = HashMap::<(i32, i32), Vec<usize>>::new();
        for (index, sprite) in static_sprites.iter().enumerate() {
            grid.entry(static_grid_cell(sprite.origin))
                .or_default()
                .push(index);
        }
        self.static_map_cache = Some(StaticMapCache {
            map_id: map.id,
            sprite_count: map.sprites.len(),
            static_sprites,
            dynamic_indices,
            grid,
        });
        self.static_visibility_cache = None;
    }

    fn visible_static_sprite_indices(
        &mut self,
        map: &Map,
        camera: [f32; 2],
        viewport: [f32; 2],
    ) -> Arc<Vec<usize>> {
        let Some(static_map) = self.static_map_cache.as_ref() else {
            return Arc::new(Vec::new());
        };
        let camera_bin = [
            (camera[0] / STATIC_GRID_SIZE).floor() as i32,
            (camera[1] / STATIC_GRID_SIZE).floor() as i32,
        ];
        let viewport_key = [viewport[0] as u32, viewport[1] as u32];
        if let Some(cache) = self.static_visibility_cache.as_ref()
            && cache.map_id == map.id
            && cache.sprite_count == static_map.static_sprites.len()
            && cache.viewport == viewport_key
            && cache.camera_bin == camera_bin
        {
            return Arc::clone(&cache.indices);
        }

        let indices = static_indices_in_view(static_map, camera, viewport, 0.0);
        let indices = Arc::new(indices);
        self.static_visibility_cache = Some(StaticVisibilityCache {
            map_id: map.id,
            sprite_count: static_map.static_sprites.len(),
            viewport: viewport_key,
            camera_bin,
            indices: Arc::clone(&indices),
        });
        indices
    }

    /// VSync on, or frames shown as fast as they are drawn: Mailbox where
    /// the driver has it (no tearing), else Immediate. Without this the
    /// monitor's refresh rate caps every frame limit.
    pub fn set_vsync(&mut self, vsync: bool) {
        let mode = if vsync {
            wgpu::PresentMode::AutoVsync
        } else if self.present_modes.contains(&wgpu::PresentMode::Mailbox) {
            wgpu::PresentMode::Mailbox
        } else if self.present_modes.contains(&wgpu::PresentMode::Immediate) {
            wgpu::PresentMode::Immediate
        } else {
            wgpu::PresentMode::AutoNoVsync
        };
        if self.config.present_mode != mode {
            self.config.present_mode = mode;
            self.surface.configure(&self.device, &self.config);
        }
    }

    /// Frees the per-moment textures (a monster's life label, a damage
    /// number, a hovered item's tooltip...) that have not been drawn for
    /// `TRANSIENT_FRAMES`. Each is named after what it shows, so without
    /// this every change of life or name left a texture behind in GPU
    /// memory. They are made again whenever they are drawn.
    fn release_unused_transients(&mut self, draws: &[Draw]) {
        self.frame_number += 1;
        let frame = self.frame_number;
        for draw in draws {
            if is_transient(&draw.texture) {
                if let Some(last) = self.transient_used.get_mut(&draw.texture) {
                    *last = frame;
                } else {
                    self.transient_used.insert(draw.texture.clone(), frame);
                }
            }
        }
        if frame % 120 != 0 {
            return;
        }
        let used = &mut self.transient_used;
        self.textures.retain(|key, _| {
            if !is_transient(key) {
                return true;
            }
            // Loaded but not drawn yet: its clock starts now.
            let last = *used.entry(key.clone()).or_insert(frame);
            frame - last < TRANSIENT_FRAMES
        });
        used.retain(|key, last| frame - *last < TRANSIENT_FRAMES && self.textures.contains_key(key));
    }

    /// Frees the textures named `prefix...` except `keep`: versions of a
    /// redrawn image that are no longer shown.
    /// The bubble over `name` while they speak, up to the left of their
    /// name (drawn centred on `origin[0]` from `name_top` down), its tail
    /// ending at the name's left, as the original's. A seller who is not
    /// speaking shows the line their booth hawks in it.
    fn push_speech(
        &mut self,
        speech: &[(String, String)],
        name: &str,
        hawk: Option<&str>,
        origin: [f32; 2],
        name_top: f32,
        viewport: [f32; 2],
        draws: &mut Vec<Draw>,
    ) -> Result<()> {
        let spoken = speech
            .iter()
            .find(|(speaker, _)| speaker.eq_ignore_ascii_case(name))
            .map(|(_, line)| line.as_str());
        let hawked = hawk.filter(|line| spoken.is_none() && !line.trim().is_empty());
        let Some(line) = spoken.or(hawked) else {
            return Ok(());
        };
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        std::hash::Hash::hash(line, &mut hasher);
        let kind = if hawked.is_some() { "hawk" } else { "say" };
        let key = format!("@speech-{kind}-{}", std::hash::Hasher::finish(&hasher));
        if !self.textures.contains_key(&key) {
            let bubble = if hawked.is_some() {
                crate::labels::booth_bubble_image(line)?
            } else {
                crate::labels::speech_bubble_image(line)?
            };
            self.upload(&key, bubble);
        }
        if let Some(texture) = self.textures.get(&key) {
            // As the original: the tip just under the life bar, a little in
            // from its left end. A booth's call stands above the name
            // instead, so the seller's name stays in sight.
            let tip = if hawked.is_some() {
                [origin[0] - 16.0, name_top - 4.0]
            } else {
                [origin[0] - 16.0, origin[1] - 107.0]
            };
            let [from_right, from_bottom] = crate::labels::SPEECH_TIP.map(|value| value as f32);
            draws.push(Draw {
                texture: key,
                vertices: quad(
                    [
                        (tip[0] + from_right - texture.size[0]).round(),
                        (tip[1] + from_bottom - texture.size[1]).round(),
                    ],
                    texture.size,
                    viewport,
                ),
                depth_test: false,
                sort_depth: WORLD_OVERLAY_SORT,
            });
        }
        Ok(())
    }

    fn prune_textures(&mut self, prefix: &str, keep: &[Option<&str>]) {
        self.textures
            .retain(|key, _| !key.starts_with(prefix) || keep.contains(&Some(key.as_str())));
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.depth = depth(&self.device, &self.config);
    }

    /// Prepare the immutable map index before the first interactive frame.
    /// Building it lazily from `draw` creates a visible one-frame hitch on
    /// maps with thousands of static sprites.
    pub fn prime_static_map_cache(&mut self, map: &Map) {
        self.ensure_static_map_cache(map);
    }

    pub fn invalidate_status(&mut self) {
        self.status_signature = None;
    }

    pub fn invalidate_hud(&mut self) {
        self.hud_signature = None;
    }

    pub fn invalidate_inventory(&mut self) {
        self.inventory_signature = None;
    }

    fn write_vertices(&mut self, vertices: &[Vertex]) {
        let required = bytemuck::cast_slice::<Vertex, u8>(vertices).len().max(1);
        if required > self.vertex_buffer_capacity {
            self.vertex_buffer_capacity = required.next_power_of_two();
            self.vertex_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("renderer dynamic vertex buffer"),
                size: self.vertex_buffer_capacity as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        self.queue
            .write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(vertices));
    }

    fn ensure_texture_loader(&mut self, assets: &Assets) {
        if self.texture_loader.is_some() {
            return;
        }
        let mut open = Some(assets.opener());
        let mut assets: Option<std::result::Result<Assets, String>> = None;
        let (requests, results) = crate::background::spawn("gravak-asset-loader", move |path: String| {
            let assets = match assets.get_or_insert_with(|| {
                open.take().expect("opened once")().map_err(|error| error.to_string())
            }) {
                Ok(assets) => assets,
                Err(message) => {
                    return LoadedTexture {
                        path,
                        data: LoadedData::Image(Err(message.clone())),
                    };
                }
            };
            let (texture, additive) = match path.strip_suffix(crate::magic::ADDITIVE_SUFFIX) {
                Some(texture) => (texture, true),
                None => (path.as_str(), false),
            };
            // A rarity's glow: the texture recoloured first.
            let (texture, tint) = match texture.split_once(crate::magic::TINT_MARK) {
                Some((texture, name)) => (texture, Some(name)),
                None => (texture, None),
            };
            let image = assets
                .image(texture)
                .map(|image| match tint {
                    Some(tint) => crate::magic::recoloured(image, tint),
                    None => image,
                })
                .map(|image| if additive { crate::magic::additive_image(image) } else { image })
                .map_err(|error| error.to_string());
            LoadedTexture {
                path,
                data: LoadedData::Image(image),
            }
        })
        .expect("failed to start asset loader thread");
        self.texture_loader = Some(TextureLoader { requests, results });
    }

    fn ensure_ui_texture_loader(&mut self, assets: &Assets) {
        if self.ui_texture_loader.is_some() {
            return;
        }
        let open = assets.opener();
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (request_tx, request_rx) = mpsc::channel::<UiTextureJob>();
            let (result_tx, result_rx) = mpsc::channel::<LoadedTexture>();
            thread::Builder::new()
                .name("gravak-ui-texture-loader".to_owned())
                .spawn(move || {
                    let mut assets = match open() {
                        Ok(assets) => assets,
                        Err(error) => {
                            let message = error.to_string();
                            while let Ok(job) = request_rx.recv() {
                                let _ = result_tx.send(LoadedTexture {
                                    path: job.path().to_owned(),
                                    data: LoadedData::Image(Err(message.clone())),
                                });
                            }
                            return;
                        }
                    };
                    let mut queue = std::collections::VecDeque::new();
                    loop {
                        // Everything asked so far, so a stale redraw can be seen.
                        if queue.is_empty() {
                            match request_rx.recv() {
                                Ok(job) => queue.push_back(job),
                                Err(_) => break,
                            }
                        }
                        while let Ok(job) = request_rx.try_recv() {
                            queue.push_back(job);
                        }
                        let Some(job) = queue.pop_front() else {
                            continue;
                        };
                        let path = job.path().to_owned();
                        let stale = job
                            .panel()
                            .is_some_and(|panel| queue.iter().any(|later| later.panel() == Some(panel)));
                        let data = if stale { LoadedData::Superseded } else { job.load(&mut assets) };
                        if result_tx.send(LoadedTexture { path, data }).is_err() {
                            break;
                        }
                    }
                })
                .expect("failed to start UI texture loader thread");
            self.ui_texture_loader = Some(UiTextureLoader {
                requests: request_tx,
                results: result_rx,
            });
        }
        // The browser runs the jobs on its loop (`background`), in order.
        #[cfg(target_arch = "wasm32")]
        {
            let mut open = Some(open);
            let mut assets: Option<std::result::Result<Assets, String>> = None;
            let (requests, results) = crate::background::spawn("gravak-ui-texture-loader", move |job: UiTextureJob| {
                let path = job.path().to_owned();
                let data = match assets.get_or_insert_with(|| {
                    open.take().expect("opened once")().map_err(|error| error.to_string())
                }) {
                    Ok(assets) => job.load(assets),
                    Err(message) => LoadedData::Image(Err(message.clone())),
                };
                LoadedTexture { path, data }
            })
            .expect("failed to start UI texture loader");
            self.ui_texture_loader = Some(UiTextureLoader { requests, results });
        }
    }

    fn ensure_geometry_loader(&mut self) {
        if self.geometry_loader.is_some() {
            return;
        }
        let (requests, results) = crate::background::spawn("gravak-geometry-loader", |job: GeometryJob| {
            let (key, vertices) = match job {
                GeometryJob::Npc { key, model, facing } => (key, model.vertices_static(facing)),
                GeometryJob::Character {
                    key,
                    character,
                    part_index,
                    facing,
                } => (key, character.vertices_idle(part_index, facing).unwrap_or_default()),
            };
            GeometryResult {
                key,
                vertices: Arc::new(vertices),
            }
        })
        .expect("failed to start geometry loader thread");
        self.geometry_loader = Some(GeometryLoader { requests, results });
    }

    fn poll_geometry_loader(&mut self) {
        let Some(loader) = &self.geometry_loader else {
            return;
        };
        while let Ok(result) = loader.results.try_recv() {
            self.geometry_loading.remove(&result.key);
            match result.key {
                GeometryKey::Npc(key) => {
                    self.static_npc_vertices.insert(key, result.vertices);
                }
                GeometryKey::Character(key) => {
                    self.static_remote_pose_vertices
                        .insert(key, result.vertices);
                }
            }
        }
    }

    fn queue_npc_geometry(
        &mut self,
        model: &crate::character::NpcModel,
        facing: f32,
    ) -> Option<Arc<Vec<([f32; 3], [f32; 2])>>> {
        let key = model.static_vertices_key(facing);
        if let Some(vertices) = self.static_npc_vertices.get(&key) {
            return Some(Arc::clone(vertices));
        }
        let geometry_key = GeometryKey::Npc(key);
        if self.geometry_loading.insert(geometry_key) {
            self.ensure_geometry_loader();
            if let Some(loader) = &self.geometry_loader {
                if loader
                    .requests
                    .send(GeometryJob::Npc {
                        key: geometry_key,
                        model: model.clone(),
                        facing,
                    })
                    .is_err()
                {
                    self.geometry_loading.remove(&geometry_key);
                }
            }
        }
        None
    }

    fn queue_character_idle_geometry(
        &mut self,
        character: &Character,
        signature: u64,
        part_index: usize,
        facing_sector: i8,
        facing: f32,
    ) -> Option<Arc<Vec<([f32; 3], [f32; 2])>>> {
        let key = (signature, facing_sector, part_index);
        if let Some(vertices) = self.static_remote_pose_vertices.get(&key) {
            return Some(Arc::clone(vertices));
        }
        let geometry_key = GeometryKey::Character(key);
        if self.geometry_loading.insert(geometry_key) {
            self.ensure_geometry_loader();
            if let Some(loader) = &self.geometry_loader {
                if loader
                    .requests
                    .send(GeometryJob::Character {
                        key: geometry_key,
                        character: character.clone(),
                        part_index,
                        facing,
                    })
                    .is_err()
                {
                    self.geometry_loading.remove(&geometry_key);
                }
            }
        }
        None
    }

    fn poll_texture_loader(&mut self) {
        if let Some(loader) = &self.texture_loader {
            while let Ok(loaded) = loader.results.try_recv() {
                self.ready_textures.push_back(loaded);
            }
        }
        // The interface's own images go up at once: a bag or a window
        // waiting behind the world's textures shows what is no longer so.
        let mut interface = Vec::new();
        if let Some(loader) = &self.ui_texture_loader {
            while let Ok(loaded) = loader.results.try_recv() {
                interface.push(loaded);
            }
        }
        for loaded in interface {
            self.loading.remove(&loaded.path);
            match loaded.data {
                LoadedData::Image(Ok(image)) => self.upload_loaded(&loaded.path, image),
                LoadedData::Image(Err(error)) => {
                    eprintln!("Skipped texture {}: {error}", loaded.path);
                    self.missing.insert(loaded.path);
                }
                LoadedData::Superseded => {}
                LoadedData::Minimap { map_id, result } => {
                    self.minimap_ready = Some((map_id, result));
                }
            }
        }
        let uploads_started = Instant::now();
        while self.texture_loads_this_frame < self.max_texture_loads_per_frame
            || uploads_started.elapsed() < TEXTURE_UPLOAD_TIME
        {
            let Some(loaded) = self.ready_textures.pop_front() else {
                break;
            };
            self.loading.remove(&loaded.path);
            self.texture_loads_this_frame += 1;
            match loaded.data {
                LoadedData::Image(Ok(image)) => self.upload_loaded(&loaded.path, image),
                LoadedData::Image(Err(error)) => {
                    eprintln!("Skipped texture {}: {error}", loaded.path);
                    self.missing.insert(loaded.path);
                }
                LoadedData::Superseded => {}
                LoadedData::Minimap { map_id, result } => {
                    self.minimap_ready = Some((map_id, result));
                }
            }
        }
    }

    fn queue_ui_texture(&mut self, assets: &Assets, job: UiTextureJob) {
        let path = job.path().to_owned();
        if self.textures.contains_key(&path)
            || self.missing.contains(&path)
            || self.loading.contains(&path)
        {
            return;
        }
        self.ensure_ui_texture_loader(assets);
        let Some(loader) = &self.ui_texture_loader else {
            return;
        };
        self.loading.insert(path.clone());
        if loader.requests.send(job).is_err() {
            self.loading.remove(&path);
            self.missing.insert(path);
        }
    }

    fn texture(&mut self, assets: &Assets, path: &str) {
        if self.textures.contains_key(path)
            || self.missing.contains(path)
            || self.loading.contains(path)
            || self.texture_requests_this_frame >= TEXTURE_REQUESTS_PER_FRAME
        {
            return;
        }
        self.ensure_texture_loader(assets);
        let Some(loader) = &self.texture_loader else {
            return;
        };
        self.loading.insert(path.to_owned());
        if loader.requests.send(path.to_owned()).is_err() {
            self.loading.remove(path);
            self.missing.insert(path.to_owned());
        } else {
            self.texture_requests_this_frame += 1;
        }
    }
    /// Keeps a picture made here, during the drawing. In the browser one made
    /// while a file it read was still on its way is dropped: it holds a
    /// stand-in, and the next frame makes it again with the file in.
    fn upload(&mut self, path: &str, image: image::RgbaImage) {
        #[cfg(target_arch = "wasm32")]
        {
            let misses = crate::web_files::misses();
            let made_without_a_file = misses != self.upload_misses;
            self.upload_misses = misses;
            if made_without_a_file {
                return;
            }
        }
        self.upload_loaded(path, image);
    }

    /// Keeps a picture from a loader, which waits for its files itself.
    fn upload_loaded(&mut self, path: &str, image: image::RgbaImage) {
        let upload_started = self.perf_enabled.then(Instant::now);
        let image_size = [image.width(), image.height()];
        if image.width() > self.device.limits().max_texture_dimension_2d
            || image.height() > self.device.limits().max_texture_dimension_2d
        {
            self.missing.insert(path.to_owned());
            return;
        }
        let texture = self.device.create_texture_with_data(
            &self.queue,
            &wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: image.width(),
                    height: image.height(),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            image.as_raw(),
        );
        let view = texture.create_view(&Default::default());
        let filter = if path == "@label" {
            wgpu::FilterMode::Nearest
        } else {
            wgpu::FilterMode::Linear
        };
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: filter,
            min_filter: filter,
            ..Default::default()
        });
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        self.textures.insert(
            path.to_owned(),
            Texture {
                bind,
                size: [image_size[0] as f32, image_size[1] as f32],
            },
        );
        if let Some(started) = upload_started {
            let elapsed = started.elapsed();
            if elapsed >= Duration::from_millis(2) {
                eprintln!(
                    "PERF_TEXTURE path={} size={}x{} upload_ms={:.2}",
                    path,
                    image_size[0],
                    image_size[1],
                    elapsed.as_secs_f64() * 1_000.0,
                );
            }
        }
    }
    /// The map pixel at the screen's top left: the hero in the middle, but
    /// never past the map's edges (as the original), where there is no
    /// picture, only black; the hero then walks off the middle.
    pub fn camera(&self, map: &Map, player: &Player) -> [f32; 2] {
        self.camera_at(map, player.position)
    }
    /// The camera with the hero on `position`.
    fn camera_at(&self, map: &Map, position: [f32; 2]) -> [f32; 2] {
        let p = map.project(position);
        let view = self.world_viewport();
        let wanted = [p[0] - view[0] * 0.5 + self.camera_shift[0], p[1] - view[1] * 0.5 + self.camera_shift[1]];
        clamp_camera(wanted, map.extent, view)
    }
    /// The part of the world the screen shows, in map pixels: the window
    /// divided by the zoom.
    pub fn world_viewport(&self) -> [f32; 2] {
        [self.config.width as f32 / self.zoom, self.config.height as f32 / self.zoom]
    }

    /// A point of the window (the cursor) where the world is drawn: in map
    /// pixels from the camera.
    pub fn world_point(&self, screen: [f32; 2]) -> [f32; 2] {
        [screen[0] / self.zoom, screen[1] / self.zoom]
    }

    /// Zooms the world in (`steps` above 0) or out by tenths, within
    /// `ZOOM_RANGE`.
    pub fn zoom_by(&mut self, steps: f32) {
        let zoom = ((self.zoom + steps * 0.1) * 10.0).round() / 10.0;
        self.zoom = zoom.clamp(*ZOOM_RANGE.start(), *ZOOM_RANGE.end());
    }

    /// The revive button under `position`: `Some(true)` revives here.
    pub fn revive_at(&self, position: [f32; 2]) -> Option<bool> {
        self.revive_rects
            .iter()
            .find(|([left, top, width, height], _)| {
                (*left..left + width).contains(&position[0]) && (*top..top + height).contains(&position[1])
            })
            .map(|(_, here)| *here)
    }

    /// A Top title circling a hero (`top_title`): the ring, the bent
    /// words with their halo and glint, sparks and embers, the aura on the
    /// ground; what passes behind the hero is drawn behind them.
    fn push_top_title(&mut self, flags: u32, origin: [f32; 2], depth: f32, time: f32, viewport: [f32; 2], draws: &mut Vec<Draw>) {
        use crate::top_title::{self, Piece};
        let Some(profession) = top_title::profession(flags, time) else {
            return;
        };
        let key = |piece: &Piece| {
            let added = if piece.added() { crate::magic::ADDITIVE_SUFFIX } else { "" };
            match piece {
                Piece::Word(word) => format!("@top-word-{profession}-{word}"),
                Piece::Glow(word) => format!("@top-glow-{profession}-{word}{added}"),
                Piece::Shine(word, frame) => format!("@top-shine-{profession}-{word}-{frame}{added}"),
                Piece::Ring => format!("@top-ring-{profession}{added}"),
                Piece::Spark => format!("@top-spark-{profession}{added}"),
                Piece::Aura => format!("@top-aura-{profession}{added}"),
            }
        };
        // The words' art first: the layout bends them by their size.
        for word in ["Top", profession] {
            let word_key = key(&Piece::Word(word.to_owned()));
            if !self.textures.contains_key(&word_key) {
                self.upload(&word_key, top_title::word_art(word, profession));
            }
        }
        let sizes: HashMap<String, [f32; 2]> = ["Top", profession]
            .into_iter()
            .filter_map(|word| Some((word.to_owned(), self.textures.get(&key(&Piece::Word(word.to_owned())))?.size)))
            .collect();
        let quads = top_title::quads(profession, origin, time, |word| sizes.get(word).copied().unwrap_or([80.0, 40.0]));
        // Pieces sharing a texture and a depth go out as one draw (the
        // ring's segments, a word's strips).
        let mut batches: Vec<(String, f32, Vec<Vertex>)> = Vec::new();
        for quad in quads {
            let texture = key(&quad.piece);
            if !self.textures.contains_key(&texture) {
                let image = match &quad.piece {
                    Piece::Word(word) => top_title::word_art(word, profession),
                    Piece::Glow(word) => top_title::word_glow(word, profession),
                    Piece::Shine(word, frame) => top_title::word_shine(word, profession, *frame),
                    Piece::Ring => top_title::ring_art(profession),
                    Piece::Spark => top_title::spark_art(profession),
                    Piece::Aura => top_title::aura_art(profession),
                };
                self.upload(&texture, image);
            }
            let layer = f32::from(quad.piece.layer()) * 0.001;
            let sort_depth = match quad.piece {
                Piece::Aura => depth - 0.04,
                _ if quad.front => depth + 0.03 + layer,
                _ => depth - 0.03 + layer,
            };
            let vertices = [0, 1, 2, 0, 2, 3]
                .map(|index| Vertex { position: clip(quad.corners[index], 0.9, viewport), uv: quad.uv[index], alpha: quad.alpha.clamp(0.0, 1.0) });
            match batches.iter_mut().find(|(key, depth, _)| *key == texture && *depth == sort_depth) {
                Some((_, _, batch)) => batch.extend(vertices),
                None => batches.push((texture, sort_depth, vertices.to_vec())),
            }
        }
        for (texture, sort_depth, vertices) in batches {
            draws.push(Draw { texture, vertices, depth_test: false, sort_depth });
        }
    }

    /// The VIP title centred over a name whose top is at `name_top`.
    fn push_vip_tag(&mut self, name_top: [f32; 2], viewport: [f32; 2], draws: &mut Vec<Draw>) -> Result<()> {
        const KEY: &str = "@vip-tag";
        if !self.textures.contains_key(KEY) {
            self.upload(KEY, crate::ornate::vip_tag(1.0)?);
        }
        if let Some(tag) = self.textures.get(KEY) {
            draws.push(Draw {
                texture: KEY.to_owned(),
                vertices: quad(
                    [(name_top[0] - tag.size[0] * 0.5).round(), (name_top[1] - tag.size[1] + 6.0).round()],
                    tag.size,
                    viewport,
                ),
                depth_test: false,
                sort_depth: WORLD_OVERLAY_SORT,
            });
        }
        Ok(())
    }

    /// Cyclone's and Superman's KOs over the head, as the original draws
    /// them: the `Ko` mark and the count in two digits, green for
    /// Cyclone and red for Superman.
    fn push_ko_tag(
        &mut self,
        assets: &mut Assets,
        ko: u32,
        flags: u64,
        name_top: [f32; 2],
        viewport: [f32; 2],
        draws: &mut Vec<Draw>,
    ) {
        const KO_MARK: &str = "@ko-mark";
        let colour = if flags & crate::network::SUPERMAN_FLAG != 0 {
            [255_u8, 0, 0]
        } else if flags & crate::network::CYCLONE_FLAG != 0 {
            [0, 255, 0]
        } else {
            return;
        };
        if !self.textures.contains_key(KO_MARK) {
            let Ok(mark) = assets.image("data/pic/ko/ko.dds") else {
                return;
            };
            self.upload(KO_MARK, mark);
        }
        let digits = format!("{:02}", ko.min(99_999));
        let mut keys = Vec::with_capacity(digits.len());
        for digit in digits.chars() {
            let key = format!("@ko-digit-{digit}-{colour:?}");
            if !self.textures.contains_key(&key) {
                let Ok(mut image) = assets.image(&format!("data/pic/Num/{digit}.dds")) else {
                    return;
                };
                for pixel in image.pixels_mut() {
                    for channel in 0..3 {
                        pixel[channel] = (u16::from(pixel[channel]) * u16::from(colour[channel]) / 255) as u8;
                    }
                }
                self.upload(&key, image);
            }
            keys.push(key);
        }
        // The mark at two thirds of its 64 pixels, the digits 18 apart.
        const MARK: f32 = 43.0;
        const DIGIT: f32 = 32.0;
        const STEP: f32 = 18.0;
        let width = MARK + keys.len() as f32 * STEP;
        let left = (name_top[0] - width * 0.5).round();
        let top = (name_top[1] - MARK).round();
        draws.push(Draw {
            texture: KO_MARK.to_owned(),
            vertices: quad([left, top], [MARK, MARK], viewport),
            depth_test: false,
            sort_depth: WORLD_OVERLAY_SORT,
        });
        for (index, key) in keys.into_iter().enumerate() {
            draws.push(Draw {
                texture: key,
                vertices: quad(
                    [left + MARK + index as f32 * STEP - 2.0, top + (MARK - DIGIT) * 0.5],
                    [DIGIT, DIGIT],
                    viewport,
                ),
                depth_test: false,
                sort_depth: WORLD_OVERLAY_SORT,
            });
        }
    }

    /// A lit XP circle's 20 seconds as `Conquer.exe` shows them
    /// (`CMyBitmap::ShowBlock`): a bar 10 pixels high at `H - 85`, from
    /// `(W - 110) * (20000 - ms) / 20000 + 100` to `W - 10`, green when
    /// full, yellow at half, red at the end.
    fn push_xp_bar(&mut self, until: Option<u64>, viewport: [f32; 2], draws: &mut Vec<Draw>) {
        const FULL_MS: f32 = 20_000.0;
        const GRADIENT: &str = "@xp-bar-gradient";
        let Some(until) = until else {
            return;
        };
        let now = crate::hud::unix_now_ms();
        if until <= now {
            return;
        }
        if !self.textures.contains_key(GRADIENT) {
            // Left to right: none of the time left (red) to all of it
            // (green), as the original works the colour out.
            let gradient = image::RgbaImage::from_fn(256, 1, |x, _| {
                let left = x as f32 / 255.0;
                let (red, green) = if left > 0.5 {
                    ((1.0 - left) / 0.5 * 255.0, 255.0)
                } else {
                    (255.0, 255.0 - (0.5 - left) / 0.5 * 255.0)
                };
                image::Rgba([red as u8, green as u8, 0, 255])
            });
            self.upload(GRADIENT, gradient);
        }
        let left_ms = ((until - now) as f32).clamp(50.0, FULL_MS);
        let left = (viewport[0] - 110.0) * (FULL_MS - left_ms) / FULL_MS + 100.0;
        let right = viewport[0] - 10.0;
        if right <= left {
            return;
        }
        let top = viewport[1] - 85.0;
        let mut vertices = quad([left, top], [right - left, 10.0], viewport);
        let u = (left_ms / FULL_MS * 255.0 + 0.5) / 256.0;
        for vertex in &mut vertices {
            vertex.uv = [u, 0.5];
        }
        draws.push(Draw {
            texture: GRADIENT.to_owned(),
            vertices,
            depth_test: false,
            sort_depth: WORLD_OVERLAY_SORT,
        });
    }

    /// The fire along the bottom (`KOfire01`): Cyclone's or Superman's
    /// time, burning from the left
    /// (all of it left) to the right edge (none), as `Conquer.exe` moves
    /// it: `x = (W - 100) * (20000 - ms) / 20000 + 100` at `H - 85`.
    fn push_xp_fire(&mut self, assets: &mut Assets, until: Option<u64>, time: f32, viewport: [f32; 2], draws: &mut Vec<Draw>) {
        const FULL_MS: f32 = 20_000.0;
        // The effect's `OffsetX`/`OffsetY` and how wide its strip is.
        const OFFSET: [f32; 2] = [460.0, -35.0];
        const STRIP: f32 = 956.0;
        let Some(until) = until else {
            return;
        };
        let now = crate::hud::unix_now_ms();
        if until <= now {
            return;
        }
        let left = ((until - now) as f32).clamp(50.0, FULL_MS);
        let x = (viewport[0] - 100.0) * (FULL_MS - left) / FULL_MS + 100.0;
        let y = viewport[1] - 85.0 + OFFSET[1];
        self.effect_scale = 1.0;
        let mut origin = x + OFFSET[0];
        // The strip repeats until it reaches the right edge.
        while origin - STRIP * 0.5 < viewport[0] {
            let mut fire = Vec::new();
            self.push_looping_effect(assets, "KOfire01", [origin, y], time, WORLD_OVERLAY_SORT, viewport, &mut fire);
            draws.extend(fire);
            origin += STRIP;
        }
        self.effect_scale = 0.75;
    }

    /// Plays `effect` once on `actor_id` (fireworks, effects of events...).
    pub fn add_role_effect(&mut self, actor_id: u32, effect: String, now: f32) {
        self.role_effects.push((actor_id, effect, now));
    }

    /// `attacker_id` shoots `flying` at `target_id`; `from_player` and
    /// `to_player` say which of them is the player.
    pub fn add_arrow(
        &mut self,
        attacker_id: u32,
        target_id: u32,
        from_player: bool,
        to_player: bool,
        flying: crate::magic::FlyingObject,
        now: f32,
    ) {
        self.arrows.push(Arrow { attacker_id, target_id, from_player, to_player, flying, started: now });
    }


    /// The arrow model `flying` at screen point `origin`, pointing along
    /// the screen direction `direction`.
    #[allow(clippy::too_many_arguments)]
    fn push_flying_object(
        &mut self,
        assets: &mut Assets,
        flying: &crate::magic::FlyingObject,
        origin: [f32; 2],
        direction: [f32; 2],
        sort_depth: f32,
        viewport: [f32; 2],
        draws: &mut Vec<Draw>,
    ) {
        if !self.flying_models.contains_key(&flying.mesh) {
            let model = assets
                .read(&flying.mesh)
                .ok()
                .and_then(|bytes| C3Document::parse(&bytes).ok())
                .map(std::rc::Rc::new);
            self.flying_models.insert(flying.mesh.clone(), model);
        }
        let Some(model) = self.flying_models.get(&flying.mesh).cloned().flatten() else {
            return;
        };
        self.texture(assets, &flying.texture);
        if !self.textures.contains_key(&flying.texture) {
            return;
        }
        let (sin, cos) = (-std::f32::consts::FRAC_PI_4).sin_cos();
        let scale = self.effect_scale * ARROW_SCALE;
        let (turn_sin, turn_cos) = arrow_turn(direction).sin_cos();
        for (mesh_index, mesh) in model.meshes.iter().enumerate() {
            let Some(motion) = model.motions.get(mesh_index).or_else(|| model.motions.first()) else {
                continue;
            };
            let vertices: Vec<Vertex> = mesh
                .vertices
                .iter()
                .map(|vertex| {
                    let p = mesh.transformed_vertex_position(vertex, motion, Matrix4::IDENTITY, 0.0);
                    let (x, y) = (p.x * turn_cos - p.y * turn_sin, p.x * turn_sin + p.y * turn_cos);
                    let screen_y = (y * sin + p.z * cos) * scale;
                    let depth = (y * cos - p.z * sin) * scale;
                    Vertex {
                        position: clip(
                            [origin[0] + x * scale, origin[1] + screen_y],
                            (0.5 + depth * 0.0005).clamp(0.001, 0.999),
                            viewport,
                        ),
                        uv: [vertex.texture_coordinate.x, vertex.texture_coordinate.y],
                        alpha: 1.0,
                    }
                })
                .collect();
            draws.push(Draw {
                texture: flying.texture.clone(),
                vertices: mesh
                    .opaque_indices
                    .iter()
                    .chain(&mesh.alpha_indices)
                    .filter_map(|index| vertices.get(*index as usize).copied())
                    .collect(),
                depth_test: false,
                sort_depth,
            });
        }
    }

    /// A soul leaves the monster slain at `from` for `killer_id`.
    pub fn add_soul(&mut self, from: [f32; 2], killer_id: u32, now: f32) {
        self.souls.push(Soul { from, killer_id, started: now });
    }

    /// Whether `position` is on the Ranking button.
    /// Whether `position` is on the Nobility Rank button.
    pub fn nobility_icon_at(&self, position: [f32; 2]) -> bool {
        self.nobility_icon_rect.is_some_and(|[left, top, width, height]| {
            (left..left + width).contains(&position[0]) && (top..top + height).contains(&position[1])
        })
    }

    /// Whether `position` is on the Helper button.
    pub fn helper_icon_at(&self, position: [f32; 2]) -> bool {
        self.helper_icon_rect.is_some_and(|[left, top, width, height]| {
            (left..left + width).contains(&position[0]) && (top..top + height).contains(&position[1])
        })
    }

    pub fn ranking_icon_at(&self, position: [f32; 2]) -> bool {
        self.ranking_icon_rect.is_some_and(|[left, top, width, height]| {
            (left..left + width).contains(&position[0]) && (top..top + height).contains(&position[1])
        })
    }

    /// Whether `position` is on the Store icon.
    pub fn store_icon_at(&self, position: [f32; 2]) -> bool {
        self.store_icon_rect.is_some_and(|[left, top, width, height]| {
            (left..left + width).contains(&position[0]) && (top..top + height).contains(&position[1])
        })
    }

    /// Whether `position` is on the bounty line under the minimap.
    pub fn bounty_tracker_at(&self, position: [f32; 2]) -> bool {
        self.bounty_tracker_rect.is_some_and(|[left, top, width, height]| {
            (left..left + width).contains(&position[0]) && (top..top + height).contains(&position[1])
        })
    }

    pub fn minimap_pointer(&mut self, position: [f32; 2]) -> bool {
        let viewport = [self.config.width as f32, self.config.height as f32];
        self.minimap_shown
            && self.minimap
                .as_mut()
                .is_some_and(|minimap| minimap.pointer(position, viewport))
    }

    /// Closes the expanded minimap; false when it was not open.
    pub fn collapse_minimap(&mut self) -> bool {
        self.minimap.as_mut().is_some_and(crate::minimap::Minimap::collapse)
    }
    pub fn draw(
        &mut self,
        assets: &mut Assets,
        map: &Map,
        character: &Character,
        action_bar: &crate::action_bar::ActionBar,
        known_skills: &std::collections::HashMap<u32, (u16, u32)>,
        skill_hud: &crate::hud::SkillHud,
        team_members: &[crate::network::TeamMember],
        team_leader_id: Option<u32>,
        team_pins: &[crate::minimap::TeamPin],
        pending_team_invite: Option<u32>,
        pending_team_join_request: Option<u32>,
        team_join_enabled: bool,
        guild_members: &[crate::network::GuildMember],
        pending_guild_invite: Option<u32>,
        trade: &crate::network::TradeView,
        action_context_slot: Option<usize>,
        player: &Player,
        remote_actors: &HashMap<u32, RemoteActor>,
        selected_target: Option<u32>,
        remote_models: &HashMap<u32, Character>,
        remote_npcs: &HashMap<u32, crate::character::NpcModel>,
        remote_monsters: &HashMap<u32, crate::character::NpcModel>,
        local_ghost: Option<&crate::character::NpcModel>,
        revive: Option<(u32, bool)>,
        combat_numbers: &[(u32, u32, f32, NumberTint)],
        skill_effects: &[SkillEffectInstance],
        ground_loot: &[(u32, [f32; 2], u32, f32)],
        level_up_until: f32,
        dialog: Option<(&[String], &[(u8, String)], u16)>,
        shop: Option<&crate::shop::ShopState>,
        equipment: &[crate::network::OwnedItem],
        hidden_default_equipment_slots: &[bool; 10],
        inventory: &[crate::network::OwnedItem],
        inventory_open: bool,
        inventory_filter: crate::shop::InventoryFilter,
        selected_inventory_uid: Option<u32>,
        warehouse: Option<&[crate::network::OwnedItem]>,
        guild_warehouse_open: bool,
        warehouse_page: usize,
        selected_warehouse_uid: Option<u32>,
        warehouse_silver: u32,
        warehouse_amount: &str,
        warehouse_amount_focused: bool,
        chat_text: &str,
        chat_focused: bool,
        chat_history: &[crate::network::ChatMessage],
        chat_filter: u16,
        network_status: &str,
        friends: &[crate::network::SocialEntry],
        ignores: &[crate::network::SocialEntry],
        pending_friend_name: Option<&str>,
        silver: u32,
        cps: u32,
        mana: u32,
        maximum_mana: u32,
        level: u8,
        experience: u64,
        nobility_donation: u64,
        nobility_rank: u32,
        nobility_remaining: u64,
        equipped_title: u32,
        nobility_ranking: &[crate::network::NobilityEntry],
        status_flags: u64,
        status_open: bool,
        status_tab: crate::shop::StatusTab,
        performance_overlay: bool,
        quick_panel: Option<crate::shop::QuickPanel>,
        hud_config: crate::hud::HudConfig,
        network_character_id: Option<u32>,
        booth: &crate::booth::BoothView,
        bounty: Option<&crate::bounty_board::BountyView>,
        store: Option<&crate::store_panel::StoreView>,
        daily: Option<&crate::daily_rewards_panel::DailyRewardsView>,
        highscores: Option<&crate::highscores_panel::HighscoresView>,
        nobility: Option<&crate::nobility_panel::NobilityView>,
        guild: Option<&crate::guild_panel::GuildView>,
        item_context: Option<crate::shop::ItemContext>,
        choice_menu: Option<crate::shop::ChoiceMenu>,
        notice: Option<&str>,
        prompt: Option<crate::shop::PromptDialog>,
        exit_confirm: Option<crate::exit_confirm::ExitConfirm>,
        forge: Option<crate::forge_window::ForgeWindow>,
        sell_all: Option<(
            &crate::sell_all::SellAllView,
            &[crate::network::OwnedItem],
            Option<crate::sell_all::SellAllAction>,
        )>,
        hovered_player: Option<u32>,
        item_action_panel: Option<crate::shop::ItemActionPanel>,
        action_animation_progress: Option<f32>,
        gem_selection_panel: Option<crate::shop::GemSelectionPanel>,
        dragging_shop_item: Option<u32>,
        dragging_inventory_item: Option<crate::network::OwnedItem>,
        cursor: [f32; 2],
        action_progress: Option<f32>,
        time: f32,
    ) -> Result<()> {
        let frame_started = Instant::now();
        let measure_perf = self.perf_enabled || performance_overlay;
        let minimap_visible = shop.is_none() && warehouse.is_none() && quick_panel.is_none();
        self.texture_loads_this_frame = 0;
        self.texture_requests_this_frame = 0;
        #[cfg(target_arch = "wasm32")]
        {
            self.upload_misses = crate::web_files::misses();
        }
        self.poll_texture_loader();
        let texture_phase_finished = Instant::now();
        self.poll_geometry_loader();
        let geometry_phase_finished = Instant::now();
        if let Some((map_id, result)) = self.minimap_ready.take() {
            if self.minimap_map == Some(map_id) {
                self.minimap = match result {
                    Ok(minimap) => minimap,
                    Err(error) => {
                        eprintln!("Minimap unavailable: {error}");
                        None
                    }
                };
                self.minimap_upload_index = 0;
                self.minimap_discovered = None;
            }
        }
        // A crypt's minimap shows only what the hero discovered.
        let mut redrawn = Vec::new();
        if let (Some(reveal), Some(minimap)) = (&self.reveal, &mut self.minimap) {
            let key = (reveal.map_id, reveal.generation);
            if reveal.map_id == map.id && self.minimap_discovered != Some(key) {
                self.minimap_discovered = Some(key);
                minimap.discovered(map, |cell| reveal.seen(cell));
                // Already up: the new pictures go up now.
                if self.minimap_upload_index > 0 {
                    redrawn = minimap
                        .images
                        .iter()
                        .take(self.minimap_upload_index)
                        .filter(|(key, _)| matches!(*key, "@minimap-crop" | "@minimap-full"))
                        .map(|(key, image)| ((*key).to_owned(), image.clone()))
                        .collect();
                }
            }
        }
        for (key, image) in redrawn {
            self.upload(&key, image);
        }
        while minimap_visible
            && self
                .minimap
                .as_ref()
                .is_some_and(|minimap| self.minimap_upload_index < minimap.images.len())
            && self.texture_loads_this_frame < self.max_texture_loads_per_frame
        {
            let (key, image) = {
                let minimap = self.minimap.as_ref().expect("minimap exists");
                let (key, image) = &minimap.images[self.minimap_upload_index];
                ((*key).to_owned(), image.clone())
            };
            self.upload(&key, image);
            self.minimap_upload_index += 1;
            self.texture_loads_this_frame += 1;
        }
        let asset_phase_finished = Instant::now();
        let camera = self.camera(map, player);
        // The world is drawn over the window as `zoom` makes it: its
        // viewport is the part of the map shown; the interface's is the
        // window (`screen`, from the minimap on).
        let screen = [self.config.width as f32, self.config.height as f32];
        let viewport = self.world_viewport();
        self.ensure_static_map_cache(map);
        let mut draws = Vec::with_capacity(map.sprites.len() / 3 + remote_actors.len() * 2 + 64);
        // A crypt: what the hero discovers from where they stand.
        if map.rooms.is_empty() {
            self.reveal = None;
        } else if self.reveal.as_ref().map(|reveal| reveal.map_id) != Some(map.id) {
            self.reveal = crate::reveal::Reveal::new(map);
        }
        if let Some(reveal) = &mut self.reveal {
            reveal.look([player.position[0].round() as i32, player.position[1].round() as i32], time);
        }
        // How much of a piece on `cell` shows: all off a crypt.
        let shown = |reveal: &Option<crate::reveal::Reveal>, cell: Option<[i32; 2]>| match (reveal, cell) {
            (Some(reveal), Some(cell)) => reveal.shown(cell, time),
            _ => 1.0,
        };

        let dynamic_count = self
            .static_map_cache
            .as_ref()
            .map_or(0, |cache| cache.dynamic_indices.len());
        for dynamic_offset in 0..dynamic_count {
            let index = self
                .static_map_cache
                .as_ref()
                .expect("static map cache exists")
                .dynamic_indices[dynamic_offset];
            let sprite = &map.sprites[index];
            let p = [sprite.origin[0] - camera[0], sprite.origin[1] - camera[1]];
            let margin = if sprite.depth == f32::NEG_INFINITY {
                256.0
            } else {
                4096.0
            };
            if p[0] > viewport[0] || p[1] > viewport[1] || p[0] < -margin || p[1] < -margin {
                continue;
            }
            let path = &sprite.frames
                [(time * 1000.0 / sprite.interval as f32) as usize % sprite.frames.len()];
            self.texture(assets, path);
            let Some(texture) = self.textures.get(path) else {
                continue;
            };
            let size = texture.size;
            if p[0] + size[0] < 0.0 || p[1] + size[1] < 0.0 {
                continue;
            }
            let alpha = shown(&self.reveal, sprite.cell);
            if alpha <= 0.0 {
                continue;
            }
            draws.push(Draw {
                texture: path.clone(),
                vertices: if alpha < 1.0 { quad_with_alpha(p, size, viewport, alpha) } else { quad(p, size, viewport) },
                depth_test: false,
                sort_depth: sprite.depth,
            });
        }
        // The hero on screen, for the walls standing in front of them.
        let hero_point = {
            let p = map.project(player.position);
            [p[0] - camera[0], p[1] - camera[1] - player.height - player.hover]
        };
        let hero_depth = player.position[0] + player.position[1];
        let visible_static_indices = self.visible_static_sprite_indices(map, camera, viewport);
        for index in visible_static_indices.iter().copied() {
            let (texture_path, origin, depth, cell) = {
                let sprite = &self
                    .static_map_cache
                    .as_ref()
                    .expect("static map cache exists")
                    .static_sprites[index];
                (sprite.texture.clone(), sprite.origin, sprite.depth, sprite.cell)
            };
            let p = [origin[0] - camera[0], origin[1] - camera[1]];
            let margin = if depth == f32::NEG_INFINITY {
                STATIC_GROUND_MARGIN
            } else {
                STATIC_OBJECT_MARGIN
            };
            if p[0] > viewport[0] || p[1] > viewport[1] || p[0] < -margin || p[1] < -margin {
                continue;
            }
            self.texture(assets, &texture_path);
            let Some(texture) = self.textures.get(&texture_path) else {
                continue;
            };
            if p[0] + texture.size[0] < 0.0 || p[1] + texture.size[1] < 0.0 {
                continue;
            }
            let size = texture.size;
            let alpha = shown(&self.reveal, cell);
            if alpha <= 0.0 {
                continue;
            }
            // A crypt's wall (any theme's) in front of the hero, over them,
            // is seen through: the walls are taller than a hero.
            let in_front = texture_path.starts_with("gravak/objects/")
                && texture_path.contains("/parede-")
                && depth > hero_depth
                && hero_point[0] > p[0] - 8.0
                && hero_point[0] < p[0] + size[0] + 8.0
                && hero_point[1] > p[1]
                && hero_point[1] - HERO_SEEN_HEIGHT < p[1] + size[1];
            let alpha = if in_front { alpha.min(WALL_SEEN_THROUGH) } else { alpha };
            draws.push(Draw {
                texture: texture_path,
                vertices: if alpha < 1.0 { quad_with_alpha(p, size, viewport, alpha) } else { quad(p, size, viewport) },
                depth_test: false,
                sort_depth: depth,
            });
        }
        // A jump shows its landing in a moment: its textures are asked now,
        // so the ground there is in when the camera gets there.
        if let Some(landing) = player.landing() {
            let landing_camera = self.camera_at(map, landing);
            let indices = self
                .static_map_cache
                .as_ref()
                .map(|static_map| static_indices_in_view(static_map, landing_camera, viewport, 0.0))
                .unwrap_or_default();
            for index in indices {
                let path = self.static_map_cache.as_ref().expect("static map cache exists").static_sprites[index]
                    .texture
                    .clone();
                self.texture(assets, &path);
            }
        }
        // A crypt's dark: over every cell not out yet, fading as it comes out.
        if self.reveal.is_some() && !self.textures.contains_key(DARK_CELL) {
            self.upload(DARK_CELL, dark_cell_image());
        }
        if let Some(reveal) = &self.reveal {
            for y in 0..map.data.height as i32 {
                for x in 0..map.data.width as i32 {
                    let dark = 1.0 - reveal.shown([x, y], time);
                    if dark <= 0.0 {
                        continue;
                    }
                    let p = map.project([x as f32, y as f32]);
                    let p = [p[0] - camera[0] - 34.0, p[1] - camera[1] - 18.0];
                    if p[0] > viewport[0] || p[1] > viewport[1] || p[0] < -68.0 || p[1] < -36.0 {
                        continue;
                    }
                    draws.push(Draw {
                        texture: DARK_CELL.to_owned(),
                        vertices: quad_with_alpha(p, [68.0, 36.0], viewport, dark),
                        depth_test: false,
                        sort_depth: DARK_SORT,
                    });
                }
            }
        }
        // Where the hero stands on screen: the middle, but off it near the
        // map's edges, where the view stops (`camera`).
        let projected = map.project(player.position);
        let ground = [projected[0] - camera[0], projected[1] - camera[1]];
        let origin = [ground[0], ground[1] - player.height - player.hover];
        let actor_depth = player.position[0] + player.position[1];
        // Each blow sweeps the weapon's flash (Action3DEffect.ini).
        if player.action == Action::Attack && player.action_instance != self.last_hero_swing {
            self.last_hero_swing = player.action_instance;
            if let Some(name) = self.swing_effect(assets, character.right_weapon_id) {
                self.weapon_flashes.push((None, name, time));
            }
        }
        self.push_weapon_flashes(assets, None, origin, player.facing, time, actor_depth + 0.05, viewport, &mut draws);
        // The world character stays visible while the Character panel is open;
        // the paper doll in the panel is a separate preview on the left.
        if time < level_up_until {
            const LEVEL_UP_DURATION: f32 = 61.0 / 30.0;
            if self.level_up_layers.is_none() {
                let layers = (1..=4)
                    .map(|number| {
                        let c3_path = format!("C3/Effect/zf2-e095/{number}.C3");
                        let texture = format!("C3/Effect/zf2-e095/{number}.dds");
                        Ok(LevelUpLayer {
                            document: C3Document::parse(&assets.read(&c3_path)?)?,
                            texture,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                self.level_up_layers = Some(layers);
            }
            for number in 1..=4 {
                let texture = format!("C3/Effect/zf2-e095/{number}.dds");
                self.texture(assets, &texture);
            }
            let elapsed =
                (time - (level_up_until - LEVEL_UP_DURATION)).clamp(0.0, LEVEL_UP_DURATION);
            let frame = elapsed * 30.0;
            let effect_origin = ground;
            let layers = self
                .level_up_layers
                .as_ref()
                .ok_or("Level-up effect unavailable")?;
            let (sin, cos) = (-std::f32::consts::FRAC_PI_4).sin_cos();
            let scale = 0.75;
            for layer in layers {
                if !self.textures.contains_key(&layer.texture) {
                    continue;
                }
                for (mesh_index, mesh) in layer.document.meshes.iter().enumerate() {
                    let Some(motion) = layer
                        .document
                        .motions
                        .get(mesh_index)
                        .or_else(|| layer.document.motions.first())
                    else {
                        continue;
                    };
                    let alpha = transparency_at(&mesh.transparency_keys, frame);
                    let vertices: Vec<Vertex> = mesh
                        .vertices
                        .iter()
                        .map(|vertex| {
                            let p = mesh.transformed_vertex_position(
                                vertex,
                                motion,
                                Matrix4::IDENTITY,
                                frame,
                            );
                            let screen_y = (p.y * sin + p.z * cos) * scale;
                            let depth = (p.y * cos - p.z * sin) * scale;
                            Vertex {
                                position: clip(
                                    [effect_origin[0] + p.x * scale, effect_origin[1] + screen_y],
                                    (0.5 + depth * 0.0005).clamp(0.001, 0.999),
                                    viewport,
                                ),
                                uv: [vertex.texture_coordinate.x, vertex.texture_coordinate.y],
                                alpha,
                            }
                        })
                        .collect();
                    let indices = mesh
                        .opaque_indices
                        .iter()
                        .chain(&mesh.alpha_indices)
                        .map(|index| vertices[*index as usize]);
                    draws.push(Draw {
                        texture: layer.texture.clone(),
                        vertices: indices.collect(),
                        depth_test: false,
                        sort_depth: actor_depth + 0.04,
                    });
                }
            }
        }
        if !self.textures.contains_key("@shadow") && !self.loading.contains("@shadow") {
            self.queue_ui_texture(
                assets,
                UiTextureJob::Shadow {
                    path: "@shadow".to_owned(),
                },
            );
        }
        if let Some(shadow) = self.textures.get("@shadow") {
            let size = [shadow.size[0] * 1.4, shadow.size[1] * 1.4];
            draws.push(Draw {
                texture: "@shadow".into(),
                vertices: quad(
                    [ground[0] - size[0] * 0.5, ground[1] - size[1] * 0.5],
                    size,
                    viewport,
                ),
                depth_test: false,
                sort_depth: actor_depth - 0.01,
            });
        }
        let local_idle = player.action == Action::Idle
            && player.progress().is_none()
            && action_progress.is_none();
        let local_signature = appearance_signature(character);
        let local_facing_sector = (player.facing / std::f32::consts::FRAC_PI_4).round() as i8;
        // A dead character, once fallen, is a ghost until revived.
        if let Some(ghost) = local_ghost {
            self.texture(assets, &ghost.texture);
            let vertices = if player.moving() {
                Arc::new(ghost.vertices_walk(time, player.facing))
            } else if let Some(vertices) = self.cached_static_npc_vertices(ghost, player.facing) {
                vertices
            } else {
                Arc::new(ghost.vertices_walk(0.0, player.facing))
            };
            let triangles = ghost
                .mesh
                .opaque_indices
                .iter()
                .chain(&ghost.mesh.alpha_indices)
                .map(|i| {
                    let (p, uv) = vertices[*i as usize];
                    Vertex {
                        position: clip(
                            [origin[0] + p[0], origin[1] + p[1]],
                            (0.5 + p[2] * 0.0005).clamp(0.001, 0.999),
                            viewport,
                        ),
                        uv,
                        alpha: 1.0,
                    }
                })
                .collect();
            draws.push(Draw {
                texture: ghost.texture.clone(),
                vertices: triangles,
                depth_test: true,
                sort_depth: actor_depth,
            });
        }
        for (part_index, part) in character.parts.iter().enumerate().filter(|_| local_ghost.is_none()) {
            self.texture(assets, &part.texture);
            let vertices = if local_idle {
                let Some(vertices) = self.queue_character_idle_geometry(
                    character,
                    local_signature,
                    part_index,
                    local_facing_sector,
                    player.facing,
                ) else {
                    continue;
                };
                vertices
            } else {
                Arc::new(character.vertices(
                    part,
                    time,
                    player.action,
                    player.facing,
                    player.progress().or(action_progress),
                ))
            };
            let triangles = part
                .mesh
                .opaque_indices
                .iter()
                .chain(&part.mesh.alpha_indices)
                .map(|i| {
                    let (p, uv) = vertices[*i as usize];
                    Vertex {
                        position: clip(
                            [origin[0] + p[0], origin[1] + p[1]],
                            (0.5 + p[2] * 0.0005).clamp(0.001, 0.999),
                            viewport,
                        ),
                        uv,
                        alpha: 1.0,
                    }
                })
                .collect();
            draws.push(Draw {
                texture: part.texture.clone(),
                vertices: triangles,
                depth_test: true,
                sort_depth: actor_depth,
            });
        }
        if local_ghost.is_none() {
            let weapons = if local_idle {
                character.weapon_matrices(0.0, Action::Idle, None)
            } else {
                character.weapon_matrices(time, player.action, player.progress().or(action_progress))
            };
            let rarity_of = |position: u8| {
                equipment.iter().find(|item| item.position == position).map_or(0, |item| item.rarity)
            };
            let rarity = [rarity_of(4), rarity_of(5)];
            self.push_weapon_auras(assets, weapons, rarity, origin, player.facing, time, actor_depth + 0.04, viewport, &mut draws);
        }
        let static_world_phase_finished = Instant::now();
        let mut remote_pose_cache =
            HashMap::<(u64, usize, i8, usize, u32), Arc<Vec<([f32; 3], [f32; 2])>>>::new();
        let mut remote_monster_pose_cache =
            HashMap::<(u32, u8, i8, u32), Arc<Vec<([f32; 3], [f32; 2])>>>::new();
        // Poses are cheap now that bones are blended once per pose; one a
        // frame left every walking monster but one sliding in its stand.
        let mut remote_pose_build_budget = 16usize;
        // Players have few parts and are few on screen; give them their own
        // budget so walking monsters cannot freeze them in the idle pose.
        let mut remote_player_pose_budget = 4usize;
        for actor in remote_actors.values() {
            // In a crypt's dark: not there for the hero yet.
            if self.reveal.as_ref().is_some_and(|reveal| {
                !reveal.seen([actor.position[0].round() as i32, actor.position[1].round() as i32])
            }) {
                continue;
            }
            let monster = !actor.npc && (400001..=499999).contains(&actor.id);
            if monster
                && should_hide_dead_monster(
                    actor.health,
                    actor.action,
                    actor.animation_until,
                    actor.respawn_until,
                    time,
                )
            {
                continue;
            }
            let held = remote_held(actor.action);
            let action = if time < actor.animation_until || held {
                remote_action(actor.action)
            } else {
                Action::Idle
            };
            // A flier keeps its flying pose, standing or gliding.
            let action = if actor.status_flags & crate::network::FLY_FLAG != 0
                && matches!(action, Action::Idle | Action::Jump | Action::WalkLeft | Action::WalkRight | Action::RunLeft | Action::RunRight)
            {
                Action::Fly
            } else {
                action
            };
            // In a jump: how far along. The jump plays to its landing, even
            // when a blow is struck in the air.
            let jump_progress = (actor.jumping
                && actor.movement_until > actor.movement_started
                && time < actor.movement_until)
                .then(|| ((time - actor.movement_started) / (actor.movement_until - actor.movement_started)).clamp(0.0, 1.0));
            let action = if jump_progress.is_some() && action == Action::Idle { Action::Jump } else { action };
            let action_progress = if action == Action::Jump && actor.action != 130 {
                jump_progress
            } else if held && time >= actor.animation_until {
                Some(1.0)
            } else {
                (action != Action::Idle && time < actor.animation_until).then(|| {
                    ((time - actor.animation_started)
                        / (actor.animation_until - actor.animation_started).max(f32::EPSILON))
                    .clamp(0.0, 1.0)
                })
            };
            let projected = map.project(actor.position);
            let mut origin = [projected[0] - camera[0], projected[1] - camera[1]];
            origin[1] -= remote_jump_lift(actor, time);
            // Fliers hover; their shadow stays on the ground.
            let flying = actor.status_flags & crate::network::FLY_FLAG != 0;
            if flying {
                origin[1] -= crate::movement::FLY_HEIGHT;
            }
            let visibility_margin = if monster {
                384.0
            } else if actor.npc {
                192.0
            } else {
                256.0
            };
            if !world_point_visible(origin, viewport, visibility_margin) {
                continue;
            }
            let depth = actor.position[0] + actor.position[1];
            let facing = -f32::from((actor.direction % 8) + 1) * std::f32::consts::FRAC_PI_4;
            // Another player's blow sweeps their weapon's flash too.
            if !actor.npc
                && actor.action == 140
                && time < actor.animation_until
                && self.last_remote_swing.get(&actor.id) != Some(&actor.animation_started)
            {
                self.last_remote_swing.insert(actor.id, actor.animation_started);
                if let Some(name) = self.swing_effect(assets, actor.right_weapon) {
                    self.weapon_flashes.push((Some(actor.id), name, time));
                }
            }
            if !actor.npc {
                self.push_weapon_flashes(assets, Some(actor.id), origin, facing, time, depth + 0.05, viewport, &mut draws);
            }
            // "Full" shadows (Options window): players and monsters get the
            // hero's ground shadow; "Simple" keeps it under the hero only.
            if hud_config.full_shadows
                && !actor.npc
                && let Some(shadow) = self.textures.get("@shadow")
            {
                let size = [shadow.size[0] * 1.4, shadow.size[1] * 1.4];
                draws.push(Draw {
                    texture: "@shadow".into(),
                    vertices: quad(
                        [origin[0] - size[0] * 0.5, origin[1] - size[1] * 0.5],
                        size,
                        viewport,
                    ),
                    depth_test: false,
                    sort_depth: depth - 0.01,
                });
            }
            if let Some(npc) = remote_npcs.get(&actor.id) {
                self.texture(assets, &npc.texture);
                let Some(vertices) = self.cached_static_npc_vertices(npc, facing) else {
                    continue;
                };
                let triangles = npc
                    .mesh
                    .opaque_indices
                    .iter()
                    .chain(&npc.mesh.alpha_indices)
                    .map(|i| {
                        let (p, uv) = vertices[*i as usize];
                        Vertex {
                            position: clip(
                                [origin[0] + p[0], origin[1] + p[1]],
                                (0.5 + p[2] * 0.0005).clamp(0.001, 0.999),
                                viewport,
                            ),
                            uv,
                            alpha: 1.0,
                        }
                    })
                    .collect();
                draws.push(Draw {
                    texture: npc.texture.clone(),
                    vertices: triangles,
                    depth_test: true,
                    sort_depth: depth,
                });
            } else if let Some(monster) = remote_monsters.get(&actor.id) {
                self.texture(assets, &monster.texture);
                // Monster C3 files contain a first motion that is the neutral
                // pose. Keep it static while idle; continuously advancing the
                // raw motion made Pheasants render with wings permanently open.
                // A respawning monster stands while the `born` effect plays
                // and it fades in; the fall pose is for deaths only.
                // Seconds since the respawn, while the effect plays.
                let born = (actor.respawn_until > time)
                    .then(|| (MONSTER_BORN_SECONDS - (actor.respawn_until - time)).max(0.0));
                let dynamic_kind = if actor.action == REMOTE_FADING && time < actor.animation_until {
                    Some(0)
                } else if time < actor.animation_until {
                        // A blow taken (320) and one missed (310) play once.
                    Some(match actor.action {
                        110 | 111 | 120 | 121 => 1,
                        320 => 3,
                        310 => 4,
                        _ => 2,
                    })
                } else {
                    None
                };
                let vertices: Arc<Vec<([f32; 3], [f32; 2])>> = if let Some(kind) = dynamic_kind {
                    let (model_id, _) = monster.static_vertices_key(facing);
                    let facing_sector = (facing / std::f32::consts::FRAC_PI_4).round() as i8;
                    // A death plays from its own start, not the clock's.
                    let (kind, frame_bucket) = if kind == 0 {
                        (10 + actor.flung, ((time - actor.animation_started).max(0.0) * 30.0).floor() as u32)
                    } else if kind >= 3 {
                        (kind, ((time - actor.animation_started).max(0.0) * 30.0).floor() as u32)
                    } else {
                        (kind, (time.max(0.0) * 30.0).floor() as u32)
                    };
                    let key = (model_id, kind, facing_sector, frame_bucket);
                    if let Some(vertices) = remote_monster_pose_cache.get(&key) {
                        Arc::clone(vertices)
                    } else {
                        if remote_pose_build_budget == 0 {
                            // Pose generation is intentionally budgeted so a
                            // crowded screen cannot spend the whole frame
                            // rebuilding C3 geometry.  It must never make the
                            // actor disappear, though: keep the last stable
                            // pose visible until a dynamic pose can be built.
                            self.cached_static_npc_vertices(monster, facing)
                                .unwrap_or_else(|| {
                                    Arc::new(match kind {
                                        10.. => monster.vertices_dying(actor.flung, time - actor.animation_started, facing),
                                        1 => monster.vertices_walk(time, facing),
                                        3 | 4 => monster.vertices_reacting(kind == 4, time - actor.animation_started, facing),
                                        _ => monster.vertices_attack(time, facing),
                                    })
                                })
                        } else {
                            remote_pose_build_budget -= 1;
                            let vertices = Arc::new(match kind {
                                10.. => monster.vertices_dying(actor.flung, time - actor.animation_started, facing),
                                1 => monster.vertices_walk(time, facing),
                                3 | 4 => monster.vertices_reacting(kind == 4, time - actor.animation_started, facing),
                                _ => monster.vertices_attack(time, facing),
                            });
                            remote_monster_pose_cache.insert(key, Arc::clone(&vertices));
                            vertices
                        }
                    }
                } else {
                    let Some(vertices) = self.cached_static_npc_vertices(monster, facing) else {
                        continue;
                    };
                    vertices
                };
                let triangles = monster
                    .mesh
                    .opaque_indices
                    .iter()
                    .chain(&monster.mesh.alpha_indices)
                    .map(|i| {
                        let (p, uv) = vertices[*i as usize];
                        Vertex {
                            position: clip(
                                [origin[0] + p[0], origin[1] + p[1]],
                                (0.5 + p[2] * 0.0005).clamp(0.001, 0.999),
                                viewport,
                            ),
                            uv,
                            alpha: born.map_or(1.0, |since| 0.25 + (since / MONSTER_FADE_IN_SECONDS).min(1.0) * 0.75),
                        }
                    })
                    .collect();
                draws.push(Draw {
                    texture: monster.texture.clone(),
                    vertices: triangles,
                    depth_test: true,
                    sort_depth: depth,
                });
                if let Some(since) = born {
                    self.push_effect(
                        assets,
                        "born",
                        origin,
                        since,
                        depth + 0.02,
                        viewport,
                        &mut draws,
                    );
                }
            } else if let Some(remote_character) = remote_models.get(&actor.id) {
                let signature = appearance_signature(remote_character);
                let facing_sector = (facing / std::f32::consts::FRAC_PI_4).round() as i8;
                let frame_bucket = action_progress.map_or_else(
                    || (time.max(0.0) * 30.0).floor() as u32,
                    |progress| (progress.clamp(0.0, 1.0) * 30.0).floor() as u32,
                );
                let animated = !(action == Action::Idle && action_progress.is_none());
                let pose_key = |part_index: usize| {
                    (
                        signature,
                        part_index,
                        facing_sector,
                        action.index(),
                        frame_bucket,
                    )
                };
                // The budget is spent per actor, never per part: posing the
                // body but not the hair or weapons left them floating in the
                // idle pose. Without budget, the whole actor stands idle.
                let cached = (0..remote_character.parts.len())
                    .all(|part_index| remote_pose_cache.contains_key(&pose_key(part_index)));
                let posed = animated && (cached || remote_player_pose_budget > 0);
                if posed && !cached {
                    remote_player_pose_budget -= 1;
                }
                for (part_index, part) in remote_character.parts.iter().enumerate() {
                    self.texture(assets, &part.texture);
                    let vertices = if !posed {
                        let Some(vertices) = self.queue_character_idle_geometry(
                            remote_character,
                            signature,
                            part_index,
                            facing_sector,
                            facing,
                        ) else {
                            continue;
                        };
                        vertices
                    } else {
                        let key = pose_key(part_index);
                        if let Some(vertices) = remote_pose_cache.get(&key) {
                            Arc::clone(vertices)
                        } else {
                            let vertices =
                                Arc::new(remote_character.vertices(part, time, action, facing, action_progress));
                            remote_pose_cache.insert(key, Arc::clone(&vertices));
                            vertices
                        }
                    };
                    let triangles = part
                        .mesh
                        .opaque_indices
                        .iter()
                        .chain(&part.mesh.alpha_indices)
                        .map(|i| {
                            let (p, uv) = vertices[*i as usize];
                            Vertex {
                                position: clip(
                                    [origin[0] + p[0], origin[1] + p[1]],
                                    (0.5 + p[2] * 0.0005).clamp(0.001, 0.999),
                                    viewport,
                                ),
                                uv,
                                alpha: 1.0,
                            }
                        })
                        .collect();
                    draws.push(Draw {
                        texture: part.texture.clone(),
                        vertices: triangles,
                        depth_test: true,
                        sort_depth: depth,
                    });
                }
                let weapons = if posed {
                    remote_character.weapon_matrices(time, action, action_progress)
                } else {
                    remote_character.weapon_matrices(0.0, Action::Idle, None)
                };
                self.push_weapon_auras(assets, weapons, actor.weapon_rarity, origin, facing, time, depth + 0.04, viewport, &mut draws);
            }
            if !actor.npc && !monster && !actor.name.is_empty() {
                self.push_speech(
                    &skill_hud.speech,
                    &actor.name,
                    actor.booth.as_deref(),
                    origin,
                    origin[1] - 152.0 + 2.0,
                    viewport,
                    &mut draws,
                )?;
            }
            // A hovered player shows their face, name and health instead
            // of the plain name.
            if hovered_player == Some(actor.id) && !actor.npc && !monster && !actor.name.is_empty() {
                let name_colour = crate::labels::vendor_or_name_colour(actor.status_flags, actor.booth.is_some());
                let key = format!(
                    "@player-plate-{}-{}-{}-{}-{:?}-{}-{}",
                    actor.id, actor.mesh, actor.health, actor.maximum_health, name_colour, actor.guild, actor.guild_rank
                );
                if !self.textures.contains_key(&key) {
                    let face = player_face(assets, actor.mesh, crate::labels::PLAYER_FACE_SIZE);
                    self.upload(
                        &key,
                        crate::labels::player_plate_with_guild(
                            face.as_ref(),
                            &actor.name,
                            name_colour,
                            actor.health,
                            actor.maximum_health,
                            &actor.guild,
                            &actor.guild_rank,
                        )?,
                    );
                }
                if let Some(plate) = self.textures.get(&key) {
                    draws.push(Draw {
                        texture: key,
                        vertices: quad(
                            [
                                (origin[0] - plate.size[0] * 0.5).round(),
                                (origin[1] - 160.0 - if actor.guild.is_empty() { 0.0 } else { 14.0 }).round(),
                            ],
                            plate.size,
                            viewport,
                        ),
                        depth_test: false,
                        sort_depth: WORLD_OVERLAY_SORT,
                    });
                }
            } else if !actor.name.is_empty()
                && (hud_config.show_names || monster)
                // The slain show neither name nor life.
                && !(monster && (actor.health == 0 || actor.action == REMOTE_FADING))
                // ShopFlags (type 16) stand nameless, as in the original.
                && !(actor.npc && actor.npc_kind == 16)
            {
                // NPCs' names in green, as the original's; a monster's tells
                // its level against the hero's.
                let name_colour = if actor.npc {
                    NPC_NAME
                } else if monster {
                    crate::labels::monster_name_colour(actor.level, u16::from(level))
                } else {
                    crate::labels::vendor_or_name_colour(actor.status_flags, actor.booth.is_some())
                };
                // The Guild War pole and gates show their life like monsters.
                let monster = monster || (actor.npc && matches!(actor.npc_kind, 10 | 26));
                let bounty_target = monster
                    && bounty
                        .and_then(|view| view.board.active())
                        .is_some_and(|task| task.monster_name == actor.name);
                let label_key = if monster {
                    format!("@monster-label-{}-{}-{bounty_target}-{:?}", actor.id, actor.health, name_colour)
                } else {
                    format!("@actor-label-{}-{:?}-{}-{}-{}", actor.id, name_colour, actor.guild, actor.guild_rank, actor.shield)
                };
                if !self.textures.contains_key(&label_key) && !self.loading.contains(&label_key) {
                    self.queue_ui_texture(
                        assets,
                        UiTextureJob::ActorLabel {
                            path: label_key.clone(),
                            name: actor.name.clone(),
                            guild: if monster || actor.npc { String::new() } else { actor.guild.clone() },
                            guild_rank: if monster || actor.npc { String::new() } else { actor.guild_rank.clone() },
                            shield: if monster || actor.npc { 0 } else { actor.shield },
                            monster,
                            bounty: bounty_target,
                            health: actor.health,
                            maximum_health: actor.maximum_health,
                            name_colour,
                        },
                    );
                }
                if let Some(label) = self.textures.get(&label_key) {
                    draws.push(Draw {
                        texture: label_key,
                        vertices: quad(
                            [
                                (origin[0] - label.size[0] * 0.5).round(),
                                (origin[1]
                                    - if actor.npc {
                                        // Over the head, not across the chest.
                                        122.0
                                    } else if monster {
                                        // Over its head: a tall creature lifts it.
                                        remote_monsters
                                            .get(&actor.id)
                                            .map_or(68.0, |model| (model.head + 4.0).max(68.0))
                                    } else {
                                        // A guild line over the name, or a shield hanging
                                        // under it, keeps the name in place.
                                        152.0 + (label.size[1] - 16.0).max(0.0)
                                            - if actor.shield == 0 { 0.0 } else { crate::labels::SHIELD_DROP as f32 }
                                    })
                                .round(),
                            ],
                            label.size,
                            viewport,
                        ),
                        depth_test: false,
                        sort_depth: WORLD_OVERLAY_SORT,
                    });
                }
                let vip = !actor.npc && !monster && actor.status_flags & VIP_FLAG != 0;
                // A guild line over the name lifts what sits over it.
                let name_top = origin[1] - 152.0 - if actor.npc || monster || actor.guild.is_empty() { 0.0 } else { 13.0 };
                if vip {
                    self.push_vip_tag([origin[0], name_top], viewport, &mut draws)?;
                }
                if !actor.npc && !monster {
                    let over_name = name_top - if vip { 24.0 } else { 0.0 };
                    self.push_ko_tag(assets, actor.ko_count, actor.status_flags, [origin[0], over_name], viewport, &mut draws);
                    let depth = actor.position[0] + actor.position[1];
                    self.push_top_title(actor.top_flags, origin, depth, time, viewport, &mut draws);
                    // The rank's insignia stays beside the noble's name.
                    if let Some((effect, _)) = crate::nobility_panel::rank_effects(actor.nobility_rank, matches!(actor.body, 1 | 2)) {
                        let beside = insignia_beside_name(origin, &actor.name);
                        self.push_looping_effect(assets, effect, beside, time, WORLD_OVERLAY_SORT, viewport, &mut draws);
                    }
                }
            }
        }
        let remote_world_phase_finished = Instant::now();
        for (index, (target_id, value, expires, tint)) in combat_numbers.iter().enumerate() {
            if *expires <= time {
                continue;
            }
            // Yellow when the hero is struck, white otherwise; a potion's
            // life green, its mana blue. A target that is gone shows
            // nothing (never on the hero).
            let hero = network_character_id == Some(*target_id);
            let Some(position) = remote_actors
                .get(target_id)
                .map(|target| target.position)
                .or(hero.then_some(player.position))
            else {
                continue;
            };
            let projected = map.project(position);
            let origin = [projected[0] - camera[0], projected[1] - camera[1]];
            if !world_point_visible(origin, viewport, 192.0) {
                continue;
            }
            // A blow of nothing shows no number: the target only dodges.
            if *value == 0 {
                continue;
            }
            let elapsed = (time - (*expires - COMBAT_NUMBER_SECONDS)) * 1_000.0;
            let digits: Vec<u32> = value.to_string().chars().filter_map(|digit| digit.to_digit(10)).collect();
            let mut keys = Vec::with_capacity(digits.len());
            let colour = match tint {
                NumberTint::Life => "green",
                NumberTint::Mana => "blue",
                NumberTint::Blow if hero => "yellow",
                NumberTint::Blow => "white",
            };
            for digit in &digits {
                let key = format!("@count-{digit}-{colour}");
                if !self.textures.contains_key(&key) {
                    let Ok(image) = assets.image(&format!("c3/effect/CountB/CountB{digit}.dds")) else {
                        break;
                    };
                    let mut image = image;
                    // The digits are white: a tint drops the channels it lacks.
                    let dropped: &[usize] = match colour {
                        "yellow" => &[2],
                        "green" => &[0, 2],
                        "blue" => &[0],
                        _ => &[],
                    };
                    for pixel in image.pixels_mut() {
                        for channel in dropped {
                            pixel.0[*channel] = 0;
                        }
                    }
                    self.upload(&key, image);
                }
                keys.push(key);
            }
            if keys.len() != digits.len() {
                continue;
            }
            let centre = [origin[0], origin[1] - 115.0 - index as f32 * 4.0];
            for (scale, rise, alpha) in combat_number_passes(elapsed) {
                let step = COUNT_SPACING * scale;
                let left = centre[0] - step * keys.len() as f32 * 0.5;
                let size = [COUNT_DIGIT[0] * scale, COUNT_DIGIT[1] * scale];
                for (place, key) in keys.iter().enumerate() {
                    let x = left + step * (place as f32 + 0.5) - size[0] * 0.5;
                    let y = centre[1] - rise - size[1] * 0.5;
                    draws.push(Draw {
                        texture: key.clone(),
                        vertices: quad_with_alpha([x, y], size, viewport, alpha),
                        depth_test: false,
                        sort_depth: WORLD_OVERLAY_SORT,
                    });
                }
            }
        }
        for effect in skill_effects {
            let Some(magic) = crate::magic::magic_type(assets, effect.skill_id, effect.level) else {
                continue;
            };
            let position_of = |id: u32| {
                remote_actors
                    .get(&id)
                    .map(|actor| actor.position)
                    .unwrap_or(player.position)
            };
            let elapsed = time - effect.started;
            // The trail (FastBlade's green sword gas): its effects one after
            // another along the line from the caster toward the aimed cell.
            if let Some(trail) = magic.trail.as_deref().filter(|_| effect.trail) {
                let caster = position_of(effect.attacker_id);
                let from = map.project(caster);
                let aimed = if effect.target_cell != (0, 0) {
                    map.project([f32::from(effect.target_cell.0), f32::from(effect.target_cell.1)])
                } else {
                    map.project(position_of(effect.target_id))
                };
                let (dx, dy) = (aimed[0] - from[0], aimed[1] - from[1]);
                let length = (dx * dx + dy * dy).sqrt();
                if length > 1.0 {
                    let (ux, uy) = (dx / length, dy / length);
                    for step in crate::magic::trail(assets, trail) {
                        let step_elapsed = elapsed - step.delay_ms as f32 / 1_000.0;
                        if step_elapsed < 0.0 {
                            continue;
                        }
                        let (along, across) = (step.along as f32, step.across as f32);
                        let origin = [
                            from[0] + ux * along - uy * across - camera[0],
                            from[1] + uy * along + ux * across - camera[1],
                        ];
                        if world_point_visible(origin, viewport, 256.0) {
                            self.push_effect(
                                assets,
                                &step.effect,
                                origin,
                                step_elapsed,
                                caster[0] + caster[1] + 0.06,
                                viewport,
                                &mut draws,
                            );
                        }
                    }
                }
            }
            // What travels to the target before it is struck: a bow skill's
            // arrow, or the skill's own projectile (MagicType fly effect).
            let caster = position_of(effect.attacker_id);
            let struck = effect.target_id != 0 && effect.target_id != effect.attacker_id;
            let target = position_of(effect.target_id);
            let cells = (target[0] - caster[0]).hypot(target[1] - caster[1]);
            // A bow skill's branch of the original shoots arrows and skips
            // the table's sender, projectile and target effects.
            let bow = magic.arrows.is_some();
            let struck = struck && !bow;
            let flight = if !struck {
                0.0
            } else if magic.fly_effect.is_some() {
                (cells / SPELL_CELLS_PER_SECOND).clamp(0.1, 1.2)
            } else {
                0.0
            };
            if let Some(fly) = magic.fly_effect.as_deref().filter(|_| struck && elapsed < flight) {
                let t = (elapsed / flight).clamp(0.0, 1.0);
                let start = map.project(caster);
                let end = map.project(target);
                let start = [start[0], start[1] - ARROW_FROM_LIFT];
                let end = [end[0], end[1] - ARROW_TO_LIFT];
                let origin = [
                    start[0] + (end[0] - start[0]) * t - camera[0],
                    start[1] + (end[1] - start[1]) * t - camera[1],
                ];
                if world_point_visible(origin, viewport, 256.0) {
                    let depth = caster[0] + caster[1] + (target[0] + target[1] - caster[0] - caster[1]) * t + 0.09;
                    self.push_looping_effect(assets, fly, origin, elapsed, depth, viewport, &mut draws);
                }
            }
            for (name, actor, delay) in [
                (magic.intone_effect.as_deref(), effect.attacker_id, 0.0),
                // Once per cast, on the caster as the skill leaves it.
                (magic.sender_effect.as_deref().filter(|_| effect.trail && !bow), effect.attacker_id, 0.0),
                // On the target once what flies to it arrives (Scatter's
                // fire-bomb where each arrow lands).
                (magic.target_effect.as_deref().filter(|_| struck), effect.target_id, flight),
            ] {
                let Some(name) = name else {
                    continue;
                };
                let since = elapsed - delay;
                if since < 0.0 {
                    continue;
                }
                let position = position_of(actor);
                let projected = map.project(position);
                let origin = [projected[0] - camera[0], projected[1] - camera[1]];
                if !world_point_visible(origin, viewport, 256.0) {
                    continue;
                }
                self.push_effect(
                    assets,
                    name,
                    origin,
                    since,
                    position[0] + position[1] + 0.05,
                    viewport,
                    &mut draws,
                );
            }
        }
        // Effects played once on an actor; each ends with its animation.
        let mut finished = Vec::new();
        for (index, (actor_id, effect, started)) in self.role_effects.clone().into_iter().enumerate() {
            let elapsed = time - started;
            let actor = remote_actors.get(&actor_id);
            let position = actor.map(|actor| actor.position).unwrap_or(player.position);
            // It stays on the body: up with a jump, up with a flight.
            let lift = match actor {
                Some(actor) => {
                    let flying = actor.status_flags & crate::network::FLY_FLAG != 0;
                    remote_jump_lift(actor, time) + if flying { crate::movement::FLY_HEIGHT } else { 0.0 }
                }
                None => player.height + player.hover,
            };
            let projected = map.project(position);
            let mut origin = [projected[0] - camera[0], projected[1] - camera[1] - lift];
            // A rank's fiery name rises over the name and its titles
            // (`3DEffect.ini` lifts the letters 145 over the hero).
            if effect.starts_with("letter") {
                origin[1] -= NOBILITY_LETTER_LIFT;
            }
            // A potion's rays rise over the drinker: 5065's own `gethp` and
            // `getmp` are small rings at the feet, so both play `magicfull`
            // (blue light rising up the body), turned red for life.
            let (shown, tint) = match effect.as_str() {
                "gethp" => ("magicfull", Some("life")),
                "getmp" => ("magicfull", None),
                other => (other, None),
            };
            self.effect_tint = tint;
            let playing = elapsed < 20.0
                && self.push_effect(assets, shown, origin, elapsed, position[0] + position[1] + 0.08, viewport, &mut draws);
            self.effect_tint = None;
            if !playing {
                finished.push(index);
            }
        }
        for index in finished.into_iter().rev() {
            self.role_effects.remove(index);
        }
        // Souls of the slain, rising and then flying to their slayer.
        self.souls
            .retain(|soul| time - soul.started < SOUL_RISE_SECONDS + SOUL_FLIGHT_SECONDS);
        for soul in self.souls.clone() {
            let killer = remote_actors
                .get(&soul.killer_id)
                .map(|actor| actor.position)
                .unwrap_or(player.position);
            let (from, to) = (map.project(soul.from), map.project(killer));
            let from = [from[0] - camera[0], from[1] - camera[1]];
            let to = [to[0] - camera[0], to[1] - camera[1]];
            let elapsed = time - soul.started;
            let path = |way: f32| soul_path(soul, from, to, way);
            if world_point_visible(from, viewport, 512.0) || world_point_visible(to, viewport, 512.0) {
                self.push_soul(path, from, elapsed, soul.from[0] + soul.from[1] + 0.07, viewport, &mut draws);
            }
        }
        // Arrows from the bows to their targets, with the effect some bows
        // give them.
        let arrows = std::mem::take(&mut self.arrows);
        let mut flying = Vec::with_capacity(arrows.len());
        for arrow in arrows {
            let place = |player_side: bool, id: u32| {
                if player_side {
                    Some(player.position)
                } else {
                    remote_actors.get(&id).map(|actor| actor.position)
                }
            };
            let to = place(arrow.to_player, arrow.target_id);
            let (Some(from), Some(to)) = (place(arrow.from_player, arrow.attacker_id), to) else {
                continue;
            };
            let elapsed = time - arrow.started - ARROW_RELEASE_SECONDS;
            let (start, end) = arrow_path(map.project(from), map.project(to));
            let duration = arrow_seconds(start, end);
            if elapsed >= duration {
                // Struck: the arrow's own effect on whom it hit.
                if let Some(effect) = arrow.flying.target_effect.clone() {
                    self.role_effects.push((arrow.target_id, effect, time));
                }
                continue;
            }
            flying.push(arrow.clone());
            if elapsed < 0.0 {
                continue;
            }
            let t = elapsed / duration;
            let origin = [
                start[0] + (end[0] - start[0]) * t - camera[0],
                start[1] + (end[1] - start[1]) * t - camera[1],
            ];
            if !world_point_visible(origin, viewport, 128.0) {
                continue;
            }
            let depth = from[0] + from[1] + (to[0] + to[1] - from[0] - from[1]) * t + 0.09;
            let direction = [end[0] - start[0], end[1] - start[1]];
            self.push_flying_object(assets, &arrow.flying, origin, direction, depth, viewport, &mut draws);
            // The bow's effect is made about the arrow (its trail along the
            // shaft): turned and sized as the arrow is.
            if let Some(effect) = &arrow.flying.effect {
                let scale = self.effect_scale;
                self.effect_yaw = Some(arrow_turn(direction));
                self.effect_scale = scale * ARROW_SCALE;
                self.push_looping_effect(assets, effect, origin, elapsed, depth, viewport, &mut draws);
                self.effect_scale = scale;
                self.effect_yaw = None;
            }
        }
        self.arrows = flying;
        // Map portals glow with the original client's `Exit` effect
        // (3DEffect.ini), which it draws on each DMap portal itself. It
        // sits under whoever stands on the portal.
        for portal in &map.data.portals {
            let position = [portal.x as f32, portal.y as f32];
            let projected = map.project(position);
            let origin = [projected[0] - camera[0], projected[1] - camera[1]];
            if !world_point_visible(origin, viewport, 192.0) {
                continue;
            }
            let depth = position[0] + position[1] - 0.5;
            self.push_looping_effect(assets, "Exit", origin, time, depth, viewport, &mut draws);
        }
        // Status effects (`ini/statuseffect.ini`) around the player and the
        // other actors: shield, Superman, Cyclone, Stigma, accuracy...
        // In the air the effects rise with whoever jumps.
        let mut status_effects = vec![(0_u32, player.position, status_flags, player.height)];
        status_effects.extend(
            remote_actors
                .iter()
                .map(|(id, actor)| (*id, actor.position, actor.status_flags, remote_jump_lift(actor, time))),
        );
        let mut trailing = std::collections::HashSet::new();
        for (id, position, flags, lift) in status_effects {
            if flags == 0 {
                continue;
            }
            let ground = map.project(position);
            let projected = [ground[0], ground[1] - lift];
            let history = self.trail_history.entry(id).or_default();
            trailing.insert(id);
            if history.front().is_none_or(|(_, last)| *last != projected) {
                history.push_front((time, projected));
            }
            while history.len() > 2 && history.back().is_some_and(|(when, _)| time - when > 2.0) {
                history.pop_back();
            }
            let origin = [projected[0] - camera[0], projected[1] - camera[1]];
            if !world_point_visible(origin, viewport, 256.0) {
                continue;
            }
            self.current_trail = (
                time,
                history.iter().map(|(when, at)| (*when, [at[0] - camera[0], at[1] - camera[1]])).collect(),
            );
            for (bit, name) in crate::magic::status_effects(assets) {
                if flags & bit != 0 {
                    self.push_looping_effect(assets, name, origin, time, position[0] + position[1] + 0.04, viewport, &mut draws);
                }
            }
            self.current_trail.1.clear();
        }
        self.trail_history.retain(|id, _| trailing.contains(id));
        for (loot_id, position, item_id, _) in ground_loot {
            let projected = map.project(*position);
            let origin = [projected[0] - camera[0], projected[1] - camera[1]];
            if !world_point_visible(origin, viewport, 96.0) {
                continue;
            }
            let key = format!("@loot-{loot_id}-{item_id}");
            if !self.textures.contains_key(&key) {
                let icon = crate::shop::ground_item_icon_image(assets, *item_id)
                    .or_else(|_| crate::labels::loot_image(*item_id));
                self.upload(&key, icon?);
            }
            let icon_size = self.textures.get(&key).map(|texture| texture.size);
            // The light of its quality under it (Super, Elite, Unique,
            // Refined; gems, DragonBalls, Meteors): a soft glow lying on the
            // ground in the quality's colour, breathing slowly, not the
            // bag's square.
            if let (Some(light), Some(size)) = (crate::shop::item_light(*item_id), icon_size) {
                let key = format!("@ground-glow-{light}{}", crate::magic::ADDITIVE_SUFFIX);
                if !self.textures.contains_key(&key) {
                    self.upload(&key, ground_glow_image(ground_glow_colour(light)));
                }
                let width = size[0].max(size[1]) * 1.6;
                let height = width * 0.5;
                let alpha = 0.6 + 0.25 * (time * 2.2).sin();
                draws.push(Draw {
                    texture: key,
                    vertices: quad_with_alpha(
                        [origin[0] - width * 0.5, origin[1] + size[1] * 0.15 - height * 0.5],
                        [width, height],
                        viewport,
                        alpha,
                    ),
                    depth_test: false,
                    sort_depth: WORLD_OVERLAY_SORT,
                });
            }
            if let Some(texture) = self.textures.get(&key) {
                draws.push(Draw {
                    texture: key,
                    vertices: quad(
                        [
                            origin[0] - texture.size[0] * 0.5,
                            origin[1] - texture.size[1] * 0.5,
                        ],
                        texture.size,
                        viewport,
                    ),
                    depth_test: false,
                    sort_depth: WORLD_OVERLAY_SORT,
                });
            }
            let name_key = format!("@loot-name-{loot_id}-{item_id}");
            if !self.textures.contains_key(&name_key) {
                let name = crate::shop::item_name(assets, *item_id);
                self.upload(&name_key, crate::labels::loot_name_image_in(&name, crate::labels::loot_colour(*item_id))?);
            }
            if let Some(name) = self.textures.get(&name_key) {
                draws.push(Draw {
                    texture: name_key,
                    vertices: quad(
                        [
                            origin[0] - name.size[0] * 0.5,
                            origin[1] + icon_size.map_or(16.0, |size| size[1] * 0.5) + 1.0,
                        ],
                        name.size,
                        viewport,
                    ),
                    depth_test: false,
                    sort_depth: WORLD_OVERLAY_SORT,
                });
            }
        }
        let world_phase_finished = Instant::now();
        let name_colour = crate::labels::vendor_or_name_colour(status_flags, character.booth.is_some());
        // The guild and its rank are drawn under the name: joining or
        // leaving one makes a new label.
        let label_key = format!(
            "@label-{}-{}-{}-{:?}-{}-{}",
            character.name, character.health, character.maximum_health, name_colour, character.guild, character.guild_rank
        );
        if !self.textures.contains_key(&label_key) && !self.loading.contains(&label_key) {
            self.queue_ui_texture(
                assets,
                UiTextureJob::Label {
                    path: label_key.clone(),
                    character: character.clone(),
                    name_colour,
                },
            );
        }
        if self.textures.contains_key(&label_key) {
            draws.push(Draw {
                texture: label_key,
                vertices: quad(
                    [(origin[0] - 128.0).round(), (origin[1] - 152.0).round()],
                    [256.0, 40.0],
                    viewport,
                ),
                depth_test: false,
                sort_depth: WORLD_OVERLAY_SORT,
            });
        }
        if status_flags & VIP_FLAG != 0 {
            let guild_line = if character.guild.is_empty() { 0.0 } else { 13.0 };
            self.push_vip_tag([origin[0], origin[1] - 152.0 - guild_line], viewport, &mut draws)?;
        }
        self.push_speech(
            &skill_hud.speech,
            &character.name,
            character.booth.as_deref(),
            origin,
            origin[1] - 152.0 + 13.0,
            viewport,
            &mut draws,
        )?;
        let over_name = origin[1] - 152.0 - if status_flags & VIP_FLAG != 0 { 24.0 } else { 0.0 };
        self.push_ko_tag(assets, skill_hud.ko_count, status_flags, [origin[0], over_name], viewport, &mut draws);
        self.push_top_title(skill_hud.top_flags, origin, actor_depth, time, viewport, &mut draws);
        if let Some((effect, _)) = crate::nobility_panel::rank_effects(skill_hud.nobility_rank, skill_hud.female) {
            let beside = insignia_beside_name(origin, &character.name);
            self.push_looping_effect(assets, effect, beside, time, WORLD_OVERLAY_SORT, viewport, &mut draws);
        }
        let viewport = screen;
        if minimap_visible && self.minimap_map != Some(map.id) {
            self.minimap_map = Some(map.id);
            self.minimap = None;
            self.minimap_ready = None;
            self.minimap_upload_index = 0;
            self.queue_ui_texture(
                assets,
                UiTextureJob::Minimap {
                    path: format!("@minimap-map-{}", map.id),
                    map_id: map.id,
                },
            );
        }
        // The Character window is not modal: the minimap and HUD stay visible
        // so world information remains available while the panel is open.
        if minimap_visible && self.minimap_shown {
            if let Some(minimap) = &self.minimap {
                for item in minimap.quads(map, player.position, team_pins, camera, viewport) {
                    let mut vertices = quad(
                        [item.rect[0], item.rect[1]],
                        [item.rect[2], item.rect[3]],
                        viewport,
                    );
                    for vertex in &mut vertices {
                        vertex.uv = [
                            item.uv[0] + vertex.uv[0] * item.uv[2],
                            item.uv[1] + vertex.uv[1] * item.uv[3],
                        ];
                    }
                    draws.push(Draw {
                        texture: item.texture.into(),
                        vertices,
                        depth_test: false,
                        sort_depth: f32::INFINITY,
                    });
                }
            }
            // Who a hovered teammate pin is, beside the cursor.
            let hovered_pin = self.minimap.as_ref().and_then(|minimap| {
                minimap.pin_at(map, player.position, team_pins, camera, viewport, cursor)
            });
            if let Some(pin) = hovered_pin.map(|index| &team_pins[index]) {
                let key = format!("@minimap-pin-name-{}", pin.name);
                if !self.textures.contains_key(&key) {
                    self.upload(&key, crate::labels::tooltip_image(&pin.name)?);
                }
                if let Some(label) = self.textures.get(&key) {
                    let x = (cursor[0] + 14.0).min(viewport[0] - label.size[0]);
                    draws.push(Draw {
                        texture: key,
                        vertices: quad([x, cursor[1] + 14.0], label.size, viewport),
                        depth_test: false,
                        sort_depth: f32::INFINITY,
                    });
                }
            }
        }
        // A lit XP circle's 20 seconds run out as a bar; the XP skill's
        // time burns as fire.
        if status_flags & (crate::network::CYCLONE_FLAG | crate::network::SUPERMAN_FLAG) != 0 {
            self.push_xp_fire(assets, skill_hud.xp_fire_until, time, viewport, &mut draws);
        } else {
            self.push_xp_bar(skill_hud.xp_fire_until, viewport, &mut draws);
        }
        let hud_hover = crate::hud::hovered_button(viewport, cursor);
        use std::hash::{Hash, Hasher};
        let mut hud_hasher = std::collections::hash_map::DefaultHasher::new();
        self.config.width.hash(&mut hud_hasher);
        self.config.height.hash(&mut hud_hasher);
        character.health.hash(&mut hud_hasher);
        character.maximum_health.hash(&mut hud_hasher);
        mana.hash(&mut hud_hasher);
        maximum_mana.hash(&mut hud_hasher);
        level.hash(&mut hud_hasher);
        experience.hash(&mut hud_hasher);
        status_flags.hash(&mut hud_hasher);
        hud_hover.hash(&mut hud_hasher);
        action_context_slot.hash(&mut hud_hasher);
        chat_text.hash(&mut hud_hasher);
        chat_focused.hash(&mut hud_hasher);
        chat_history.hash(&mut hud_hasher);
        chat_filter.hash(&mut hud_hasher);
        network_status.hash(&mut hud_hasher);
        action_bar.hash(&mut hud_hasher);
        hud_config.hash(&mut hud_hasher);
        let mut hud_skills = skill_hud.clone();
        hud_skills.hovered = crate::hud::skill_bar_at(assets, viewport, cursor, &hud_skills);
        hud_skills.hash(&mut hud_hasher);
        let hud_signature = hud_hasher.finish();
        let hud_key = format!("@hud-{hud_signature}");
        if self.hud_signature != Some(hud_signature) {
            self.queue_ui_texture(
                assets,
                UiTextureJob::Hud {
                    path: hud_key.clone(),
                    width: crate::hud::render_width(self.config.width),
                    height: crate::hud::render_height(self.config.height),
                    character_name: character.name.clone(),
                    health: character.health,
                    max_health: character.maximum_health,
                    mana,
                    max_mana: maximum_mana,
                    level,
                    experience,
                    action_bar: action_bar.clone(),
                    hover: hud_hover,
                    action_context_slot,
                    chat_text: chat_text.to_owned(),
                    chat_focused,
                    chat_history: chat_history.to_vec(),
                    chat_filter,
                    double_experience_active: status_flags
                        & crate::network::DOUBLE_EXPERIENCE_FLAG
                        != 0,
                    heaven_blessing_active: status_flags & crate::network::HEAVEN_BLESSING_FLAG
                        != 0,
                    burning_active: status_flags & crate::network::POISONED_FLAG != 0,
                    stunned_active: status_flags & crate::network::STUNNED_FLAG != 0,
                    cyclone_active: status_flags & crate::network::CYCLONE_FLAG != 0,
                    superman_active: status_flags & crate::network::SUPERMAN_FLAG != 0,
                    stigma_active: status_flags & crate::network::STIGMA_FLAG != 0,
                    shield_active: status_flags & crate::network::SHIELD_FLAG != 0,
                    hud_config,
                    skills: hud_skills,
                },
            );
            self.hud_signature = Some(hud_signature);
        }
        let hud_draw_key = if self.textures.contains_key(&hud_key) {
            self.last_hud_key = Some(hud_key.clone());
            Some(hud_key.clone())
        } else {
            self.last_hud_key.clone()
        };
        // The HUD is redrawn whenever its contents change (the FPS every
        // second, the position every step); only the shown version and the
        // one on its way may stay in GPU memory.
        let shown_hud = self.last_hud_key.clone();
        self.prune_textures("@hud-", &[Some(hud_key.as_str()), shown_hud.as_deref()]);
        if let Some(hud_draw_key) = hud_draw_key
            && let Some(texture) = self.textures.get(&hud_draw_key)
        {
            draws.push(Draw {
                texture: hud_draw_key,
                vertices: quad(crate::hud::position(viewport), texture.size, viewport),
                depth_test: false,
                sort_depth: f32::INFINITY,
            });
        }
        // The chat log, over the bar's corner.
        if let Some(view) = self.chat_log.clone() {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            std::hash::Hash::hash(&view, &mut hasher);
            let key = format!("@chat-log-{}", std::hash::Hasher::finish(&hasher));
            if !self.textures.contains_key(&key) {
                let image = crate::chat_log::image(&view)?;
                self.prune_textures("@chat-log-", &[]);
                self.upload(&key, image);
            }
            if let Some(texture) = self.textures.get(&key) {
                let [x, y, _, _] = self.chat_log_rect;
                draws.push(Draw {
                    texture: key,
                    vertices: quad([x as f32, y as f32], texture.size, viewport),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
        }
        // The original's animated status flags (double EXP, Heaven's
        // Blessing, lucky time) in their slots of the status bar.
        let hud_origin = crate::hud::position(viewport);
        for (effect, centre) in crate::hud::status_flag_slots(skill_hud) {
            let centre = [hud_origin[0] + centre[0], hud_origin[1] + centre[1]];
            self.push_ui_effect(assets, effect, centre, time, 30.0, viewport, &mut draws);
        }
        if show_feedback(network_status) {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            network_status.hash(&mut hasher);
            let signature = hasher.finish();
            let feedback_key = format!("@network-feedback-{signature}");
            if self.feedback_signature != Some(signature) {
                self.queue_ui_texture(
                    assets,
                    UiTextureJob::Feedback {
                        path: feedback_key.clone(),
                        message: network_status.to_owned(),
                    },
                );
                self.feedback_signature = Some(signature);
                self.feedback_since = time;
            }
            let fresh = time - self.feedback_since < FEEDBACK_SECONDS || debug_status();
            if fresh && let Some(texture) = self.textures.get(&feedback_key) {
                draws.push(Draw {
                    texture: feedback_key,
                    vertices: quad(
                        [(viewport[0] - texture.size[0]) * 0.5, 12.0],
                        texture.size,
                        viewport,
                    ),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
        }
        // Dead: the revive countdown above the KO counter, then the
        // original plates side by side there: Revive, and Revive Here,
        // which only Heaven's Blessing lets the player use.
        self.revive_rects.clear();
        let [right, bottom] = crate::hud::revive_anchor(viewport);
        let mut live_keys: Vec<String> = Vec::new();
        match revive {
            Some((remaining, _)) if remaining > 0 => {
                let key = format!("@revive-{remaining}");
                if !self.textures.contains_key(&key) {
                    self.upload(&key, crate::hud::revive_countdown_image(remaining)?);
                }
                if let Some(texture) = self.textures.get(&key) {
                    draws.push(Draw {
                        texture: key.clone(),
                        vertices: quad([right - texture.size[0], bottom - texture.size[1]], texture.size, viewport),
                        depth_test: false,
                        sort_depth: f32::INFINITY,
                    });
                }
                live_keys.push(key);
            }
            Some((_, blessed)) => {
                // From the right: Revive Here, then Revive to its left.
                let mut left = right;
                for here in [true, false] {
                    let usable = !here || blessed;
                    let base = format!("@revive-plate-{here}-{usable}");
                    let hovered = self.textures.get(&format!("{base}-false")).is_some_and(|texture| {
                        let [width, height] = texture.size;
                        (left - width..left).contains(&cursor[0]) && (bottom - height..bottom).contains(&cursor[1])
                    });
                    let lit = hovered && usable;
                    for lit in [false, lit] {
                        let key = format!("{base}-{lit}");
                        if !self.textures.contains_key(&key) {
                            self.upload(&key, crate::hud::revive_plate(assets, here, lit, usable)?);
                        }
                        live_keys.push(key);
                    }
                    let key = format!("{base}-{lit}");
                    if let Some(texture) = self.textures.get(&key) {
                        let position = [left - texture.size[0], bottom - texture.size[1]];
                        self.revive_rects.push(([position[0], position[1], texture.size[0], texture.size[1]], here));
                        draws.push(Draw {
                            texture: key,
                            vertices: quad(position, texture.size, viewport),
                            depth_test: false,
                            sort_depth: f32::INFINITY,
                        });
                        left = position[0] - 4.0;
                    }
                }
            }
            None => {}
        }
        let live: Vec<Option<&str>> = live_keys.iter().map(|key| Some(key.as_str())).collect();
        self.prune_textures("@revive-", &live);
        // The bounty under way, counted under the minimap.
        // The icon shows even before the board arrives.
        let tracker = Some(bounty.map_or(
            crate::bounty_board::Tracker { hunt: None, daily_available: false },
            |view| crate::bounty_board::tracker(&view.board),
        ));
        let tracker_hovered = self.bounty_tracker_at(cursor);
        let tracker_key = tracker.as_ref().map(|tracker| {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            tracker.hash(&mut hasher);
            tracker_hovered.hash(&mut hasher);
            format!("@bounty-tracker-{}", hasher.finish())
        });
        if let (Some(tracker), Some(key)) = (&tracker, &tracker_key) {
            if !self.textures.contains_key(key) {
                self.upload(key, crate::bounty_board::tracker_image(assets, tracker, tracker_hovered)?);
            }
            let top = match &self.minimap {
                _ if !self.minimap_shown => 0.0,
                Some(minimap) => minimap.bottom(viewport),
                None => 180.0,
            } + 8.0;
            if let Some(texture) = self.textures.get(key) {
                let left = viewport[0] - texture.size[0] - 8.0;
                self.bounty_tracker_rect = Some([left, top, texture.size[0], texture.size[1]]);
                draws.push(Draw {
                    texture: key.clone(),
                    vertices: quad([left, top], texture.size, viewport),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
            // The icon's name under the cursor.
            if tracker.hunt.is_none() && tracker_hovered {
                let tip = crate::bounty_board::icon_tooltip(tracker);
                let tip_key = format!("@bounty-tip-{tip}");
                if !self.textures.contains_key(&tip_key) {
                    self.upload(&tip_key, crate::labels::tooltip_image(tip)?);
                }
                if let (Some(texture), Some([left, top, _, height])) =
                    (self.textures.get(&tip_key), self.bounty_tracker_rect)
                {
                    draws.push(Draw {
                        texture: tip_key.clone(),
                        vertices: quad(
                            [left - texture.size[0] - 6.0, top + (height - texture.size[1]) * 0.5],
                            texture.size,
                            viewport,
                        ),
                        depth_test: false,
                        sort_depth: f32::INFINITY,
                    });
                }
            }
        }
        self.prune_textures("@bounty-tracker-", &[tracker_key.as_deref()]);
        // The Rift run under way, under the bounty's tracker.
        let rift = self
            .rift
            .as_ref()
            .and_then(|(state, since)| crate::rift_tracker::Tracker::now(state, since.elapsed().as_secs_f32()));
        let rift_key = rift.as_ref().map(|rift| {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            rift.hash(&mut hasher);
            format!("@rift-tracker-{}", hasher.finish())
        });
        if let (Some(rift), Some(key)) = (&rift, &rift_key) {
            if !self.textures.contains_key(key) {
                self.upload(key, crate::rift_tracker::image(assets, rift)?);
            }
            let top = self.bounty_tracker_rect.map_or(190.0, |[_, top, _, height]| top + height + 6.0);
            if let Some(texture) = self.textures.get(key) {
                let left = viewport[0] - texture.size[0] - 8.0;
                draws.push(Draw {
                    texture: key.clone(),
                    vertices: quad([left, top], texture.size, viewport),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
        }
        self.prune_textures("@rift-tracker-", &[rift_key.as_deref()]);
        // The Store button, past the quiver over the bar.
        let [left, top, width, height] = crate::hud::store_button_rect(viewport);
        self.store_icon_rect = Some([left, top, width, height]);
        let store_hovered = self.store_icon_at(cursor);
        let store_key = if store_hovered { "@store-button-hover" } else { "@store-button" };
        if !self.textures.contains_key(store_key) {
            self.upload(store_key, crate::store_panel::button_image(assets, store_hovered)?);
        }
        if self.textures.contains_key(store_key) {
            draws.push(Draw {
                texture: store_key.to_owned(),
                vertices: quad([left, top], [width, height], viewport),
                depth_test: false,
                sort_depth: f32::INFINITY,
            });
        }
        if store_hovered {
            let tip_key = "@store-tip";
            if !self.textures.contains_key(tip_key) {
                self.upload(tip_key, crate::labels::tooltip_image("Store (P)")?);
            }
            if let Some(tip) = self.textures.get(tip_key) {
                draws.push(Draw {
                    texture: tip_key.to_owned(),
                    vertices: quad([left, top - tip.size[1] - 4.0], tip.size, viewport),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
        }
        // The Ranking button, beside the Store's.
        let [left, top, width, height] = crate::hud::ranking_button_rect(viewport);
        self.ranking_icon_rect = Some([left, top, width, height]);
        self.nobility_icon_rect = skill_hud.info.is_some().then(|| crate::hud::nobility_button_rect(skill_hud));
        let ranking_hovered = self.ranking_icon_at(cursor);
        let ranking_key = if ranking_hovered { "@ranking-button-hover" } else { "@ranking-button" };
        if !self.textures.contains_key(ranking_key) {
            self.upload(
                ranking_key,
                crate::emblems::round_button(crate::emblems::Feature::Ranking, ranking_hovered),
            );
        }
        if self.textures.contains_key(ranking_key) {
            draws.push(Draw {
                texture: ranking_key.to_owned(),
                vertices: quad([left, top], [width, height], viewport),
                depth_test: false,
                sort_depth: f32::INFINITY,
            });
        }
        if ranking_hovered {
            let tip_key = "@ranking-tip";
            if !self.textures.contains_key(tip_key) {
                self.upload(tip_key, crate::labels::tooltip_image("Highscores (H)")?);
            }
            if let Some(tip) = self.textures.get(tip_key) {
                draws.push(Draw {
                    texture: tip_key.to_owned(),
                    vertices: quad([left, top - tip.size[1] - 4.0], tip.size, viewport),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
        }
        // The Helper button, beside the Ranking's; its light says whether
        // the helper is on.
        let [left, top, width, height] = crate::hud::helper_button_rect(viewport);
        self.helper_icon_rect = Some([left, top, width, height]);
        let helper_hovered = self.helper_icon_at(cursor);
        let helper_key = format!("@helper-button-{}-{}", u8::from(helper_hovered), u8::from(self.helper_on));
        if !self.textures.contains_key(&helper_key) {
            self.upload(&helper_key, crate::emblems::helper_button(helper_hovered, self.helper_on));
        }
        if self.textures.contains_key(&helper_key) {
            draws.push(Draw {
                texture: helper_key,
                vertices: quad([left, top], [width, height], viewport),
                depth_test: false,
                sort_depth: f32::INFINITY,
            });
        }
        if helper_hovered {
            let tip_key = format!("@helper-tip-{}", self.helper_tip);
            if !self.textures.contains_key(&tip_key) {
                self.upload(&tip_key, crate::labels::tooltip_image(&self.helper_tip)?);
            }
            if let Some(tip) = self.textures.get(&tip_key) {
                draws.push(Draw {
                    texture: tip_key.clone(),
                    vertices: quad([left, top - tip.size[1] - 4.0], tip.size, viewport),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
        }
        // Each status message is its own texture; keep only the latest.
        let current_feedback = self
            .feedback_signature
            .map(|signature| format!("@network-feedback-{signature}"));
        self.prune_textures("@network-feedback-", &[current_feedback.as_deref()]);
        // The hint of the round button under the cursor, over it.
        if let Some(button) = crate::original_ui_layout::main_hud_button_at(viewport, cursor)
            && let Some(tip) = crate::hud::round_button_tip(button, skill_hud.shift_screen)
        {
            let tip_key = format!("@round-button-tip-{tip}");
            if !self.textures.contains_key(&tip_key) {
                self.upload(&tip_key, crate::labels::tooltip_image(tip)?);
            }
            if let Some(texture) = self.textures.get(&tip_key) {
                let [x, y] = crate::original_ui_layout::main_hud_position(viewport);
                let rect = button.local_rect();
                let left = (x + rect.x + rect.width * 0.5 - texture.size[0] * 0.5).max(0.0);
                draws.push(Draw {
                    texture: tip_key,
                    vertices: quad([left, y + rect.y - texture.size[1] - 4.0], texture.size, viewport),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
        }
        // The experience bar under the cursor: how far into the level.
        if crate::original_ui_layout::exp_bar_at(viewport, cursor) {
            let line = crate::leveling::progress_line(level, experience);
            let tip_key = format!("@exp-tip-{line}");
            self.prune_textures("@exp-tip-", &[Some(tip_key.as_str())]);
            if !self.textures.contains_key(&tip_key) {
                self.upload(&tip_key, crate::labels::tooltip_image(&line)?);
            }
            if let Some(texture) = self.textures.get(&tip_key) {
                let left = (cursor[0] - texture.size[0] * 0.5).clamp(0.0, (viewport[0] - texture.size[0]).max(0.0));
                let bar = crate::original_ui_layout::main_hud_position(viewport);
                let top = bar[1] + crate::original_ui_layout::EXP_RECT.y - texture.size[1] - 8.0;
                draws.push(Draw {
                    texture: tip_key,
                    vertices: quad([left, top], texture.size, viewport),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
        }
        if let Some(view) = &skill_hud.viewed_equipment {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            view.hash(&mut hasher);
            let key = format!("@view-equipment-{}", hasher.finish());
            self.prune_textures("@view-equipment-", &[Some(key.as_str())]);
            if !self.textures.contains_key(&key) {
                let image = crate::view_equipment::image(assets, view);
                self.upload(&key, image);
            }
            let [left, top] = crate::view_equipment::position(viewport);
            if let Some(texture) = self.textures.get(&key) {
                draws.push(Draw {
                    texture: key,
                    vertices: quad([left, top], texture.size, viewport),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
            // What the item under the cursor is, beside it.
            if let Some(item) = crate::view_equipment::item_at(view, [cursor[0] - left, cursor[1] - top]) {
                let tooltip_key = format!(
                    "@viewed-item-tip-{}-{}-{}-{}-{}-{}-{}-{}-{}",
                    item.uid,
                    item.item_type,
                    item.durability,
                    item.max_durability,
                    item.plus,
                    item.gem_one,
                    item.gem_two,
                    item.rarity,
                    item.bless
                );
                if !self.textures.contains_key(&tooltip_key) {
                    let info = crate::shop::item_info(assets, item.item_type);
                    self.upload(
                        &tooltip_key,
                        crate::shop::equipment_tooltip_image(
                            item.uid,
                            item.item_type,
                            info.as_ref(),
                            item.durability,
                            item.max_durability,
                            item.plus,
                            item.gem_one,
                            item.gem_two,
                            item.rarity,
                            item.affixes,
                            item.bless,
                            item.enchant_hp,
                        )?,
                    );
                }
                if let Some(texture) = self.textures.get(&tooltip_key) {
                    // To the left of the dialog, which stands at the right.
                    let position = [
                        (cursor[0] - texture.size[0] - 16.0).max(0.0),
                        (cursor[1] + 16.0).min(viewport[1] - texture.size[1]).max(0.0),
                    ];
                    draws.push(Draw {
                        texture: tooltip_key,
                        vertices: quad(position, texture.size, viewport),
                        depth_test: false,
                        sort_depth: f32::INFINITY,
                    });
                }
            }
        }
        if skill_hud.action_panel_open {
            let key = format!("@action-panel-{:?}", skill_hud.action_panel_hover);
            if !self.textures.contains_key(&key) {
                let image = crate::action_panel::image(assets, skill_hud.action_panel_hover)?;
                self.upload(&key, image);
            }
            if let Some(texture) = self.textures.get(&key) {
                draws.push(Draw {
                    texture: key,
                    vertices: quad(crate::action_panel::position(viewport), texture.size, viewport),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
        }
        if status_open {
            use std::hash::{Hash, Hasher};
            let status_equipment = crate::shop::status_equipment_items(
                character,
                equipment,
                hidden_default_equipment_slots,
            );
            let hovered_equipment_slot = (status_tab == crate::shop::StatusTab::Equipment)
                .then(|| crate::shop::status_equipment_slot_at(viewport, cursor))
                .flatten();
            let hovered_equipment = hovered_equipment_slot.and_then(|slot| {
                status_equipment
                    .iter()
                    .find(|item| item.position == slot)
                    .copied()
            });
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            level.hash(&mut hasher);
            experience.hash(&mut hasher);
            nobility_donation.hash(&mut hasher);
            nobility_rank.hash(&mut hasher);
            nobility_remaining.hash(&mut hasher);
            equipped_title.hash(&mut hasher);
            nobility_ranking.hash(&mut hasher);
            character.health.hash(&mut hasher);
            character.maximum_health.hash(&mut hasher);
            character.name.hash(&mut hasher);
            character.body_id.hash(&mut hasher);
            character.armor_id.hash(&mut hasher);
            character.hair_id.hash(&mut hasher);
            character.right_weapon_id.hash(&mut hasher);
            character.left_weapon_id.hash(&mut hasher);
            // Everything else the panel writes: a promotion changes only the
            // class, and the texture is keyed by this signature.
            character.helmet_id.hash(&mut hasher);
            character.guild.hash(&mut hasher);
            character.guild_rank.hash(&mut hasher);
            character.strength.hash(&mut hasher);
            character.agility.hash(&mut hasher);
            character.vitality.hash(&mut hasher);
            character.spirit.hash(&mut hasher);
            character.attribute_points.hash(&mut hasher);
            character.current_class.hash(&mut hasher);
            character.previous_class.hash(&mut hasher);
            character.rebirths.hash(&mut hasher);
            character.kill_points.hash(&mut hasher);
            character.spouse.hash(&mut hasher);
            status_equipment.hash(&mut hasher);
            status_tab.hash(&mut hasher);
            let mut status_skills = known_skills
                .iter()
                .map(|(&id, &(level, experience))| (id, level, experience))
                .collect::<Vec<_>>();
            status_skills.sort_unstable();
            status_skills.hash(&mut hasher);
            skill_hud.weapon_skills.hash(&mut hasher);
            skill_hud.weapon_filter.hash(&mut hasher);
            skill_hud.active_skill.hash(&mut hasher);
            skill_hud.quiver.hash(&mut hasher);
            skill_hud.map_shown.hash(&mut hasher);
            skill_hud.shift_screen.hash(&mut hasher);
            skill_hud.picking_equipment.hash(&mut hasher);
            skill_hud.skill_page.hash(&mut hasher);
            skill_hud.attribute_details.hash(&mut hasher);
            mana.hash(&mut hasher);
            maximum_mana.hash(&mut hasher);
            let close_hovered = crate::shop::status_close_at(viewport, cursor);
            let reputation_donate_hovered = status_tab == crate::shop::StatusTab::Reputation
                && crate::shop::status_reputation_donate_at(viewport, cursor);
            let title_hovered = (status_tab == crate::shop::StatusTab::Title)
                .then(|| crate::shop::status_title_at(viewport, cursor))
                .flatten();
            close_hovered.hash(&mut hasher);
            reputation_donate_hovered.hash(&mut hasher);
            title_hovered.hash(&mut hasher);
            let signature = hasher.finish();
            let weapon_filter = skill_hud.weapon_filter;
            let attribute_details = skill_hud.attribute_details;
            let base_key = format!(
                "@original-character-base-{status_tab:?}-{weapon_filter:?}-{close_hovered}-{attribute_details}"
            );
            let dynamic_key = format!("@original-character-dynamic-{signature}");
            if !self.textures.contains_key(&base_key) && !self.loading.contains(&base_key) {
                self.queue_ui_texture(
                    assets,
                    UiTextureJob::StatusBase {
                        path: base_key.clone(),
                        active_tab: status_tab,
                        weapon_filter,
                        close_hovered,
                        attribute_details,
                    },
                );
            }
            if self.status_signature != Some(signature) {
                self.queue_ui_texture(
                    assets,
                    UiTextureJob::StatusDynamic {
                        path: dynamic_key.clone(),
                        level,
                        experience,
                        nobility_rank,
                        character: character.clone(),
                        active_tab: status_tab,
                        mana,
                        maximum_mana,
                        known_skills: status_skills.clone(),
                        equipment: status_equipment.clone(),
                        weapon_skills: skill_hud.weapon_skills.clone(),
                        weapon_filter,
                        skill_page: skill_hud.skill_page,
                        attribute_details,
                    },
                );
                self.status_signature = Some(signature);
            }
            if skill_hud.battle_power_open {
                let power = crate::shop::BattlePower::of(level, character.rebirths, &status_equipment, nobility_rank);
                let key = format!("@battle-power-{power:?}-{}-{level}", character.rebirths);
                if !self.textures.contains_key(&key) {
                    let image = crate::battle_power_panel::evaluation_image(assets, &power, character.rebirths, level)?;
                    self.prune_textures("@battle-power-", &[]);
                    self.upload(&key, image);
                }
                if let Some(texture) = self.textures.get(&key) {
                    draws.push(Draw {
                        texture: key,
                        vertices: quad(crate::battle_power_panel::evaluation_position(viewport), texture.size, viewport),
                        depth_test: false,
                        sort_depth: f32::INFINITY,
                    });
                }
                if let Some(part) = skill_hud.battle_power_detail {
                    let mut detail_hasher = std::collections::hash_map::DefaultHasher::new();
                    status_equipment.hash(&mut detail_hasher);
                    let key = format!("@battle-detail-{part}-{power:?}-{}", detail_hasher.finish());
                    if !self.textures.contains_key(&key) {
                        let image = crate::battle_power_panel::details_image(assets, &status_equipment, &power, part)?;
                        self.prune_textures("@battle-detail-", &[]);
                        self.upload(&key, image);
                    }
                    if let Some(texture) = self.textures.get(&key) {
                        draws.push(Draw {
                            texture: key,
                            vertices: quad(crate::battle_power_panel::details_position(viewport), texture.size, viewport),
                            depth_test: false,
                            sort_depth: f32::INFINITY,
                        });
                    }
                }
            }
            let status_base_draw_key = if self.textures.contains_key(&base_key) {
                self.last_status_base_key = Some(base_key.clone());
                Some(base_key.clone())
            } else {
                self.last_status_base_key.clone()
            };
            if let Some(status_base_draw_key) = status_base_draw_key
                && let Some(texture) = self.textures.get(&status_base_draw_key)
            {
                draws.push(Draw {
                    texture: status_base_draw_key,
                    vertices: quad(
                        crate::shop::status_position(viewport),
                        texture.size,
                        viewport,
                    ),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
            let status_dynamic_draw_key = if self.textures.contains_key(&dynamic_key) {
                self.last_status_dynamic_key = Some(dynamic_key.clone());
                Some(dynamic_key.clone())
            } else {
                self.last_status_dynamic_key.clone()
            };
            if let Some(status_dynamic_draw_key) = status_dynamic_draw_key
                && let Some(texture) = self.textures.get(&status_dynamic_draw_key)
            {
                draws.push(Draw {
                    texture: status_dynamic_draw_key,
                    vertices: quad(
                        crate::shop::status_position(viewport),
                        texture.size,
                        viewport,
                    ),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
            // The native 5065 character paper doll belongs to the Equipment
            // view. Role/Weapon/Mission use their own original panels and do
            // not render the live model underneath their text/list controls.
            if status_tab == crate::shop::StatusTab::Equipment {
                self.push_status_character(
                    assets,
                    character,
                    crate::shop::status_position(viewport),
                    viewport,
                    time,
                    &mut draws,
                );
            }
            if status_tab == crate::shop::StatusTab::Equipment {
                let mut overlay_hasher = std::collections::hash_map::DefaultHasher::new();
                status_equipment.hash(&mut overlay_hasher);
                hovered_equipment_slot.hash(&mut overlay_hasher);
                let overlay_key = format!("@status-equipment-overlay-{}", overlay_hasher.finish());
                if !self.textures.contains_key(&overlay_key) && !self.loading.contains(&overlay_key)
                {
                    self.queue_ui_texture(
                        assets,
                        UiTextureJob::StatusOverlay {
                            path: overlay_key.clone(),
                            equipment: status_equipment.clone(),
                            hovered_slot: hovered_equipment_slot,
                        },
                    );
                }
                let lights = crate::shop::status_equipment_lights(&status_equipment);
                self.push_item_lights(assets, &lights, crate::shop::status_position(viewport), time, viewport, &mut draws);
                let overlay_draw_key = if self.textures.contains_key(&overlay_key) {
                    self.last_status_overlay_key = Some(overlay_key.clone());
                    Some(overlay_key.clone())
                } else {
                    self.last_status_overlay_key.clone()
                };
                if let Some(overlay_draw_key) = overlay_draw_key
                    && let Some(texture) = self.textures.get(&overlay_draw_key)
                {
                    draws.push(Draw {
                        texture: overlay_draw_key,
                        vertices: quad(
                            crate::shop::status_position(viewport),
                            texture.size,
                            viewport,
                        ),
                        depth_test: false,
                        sort_depth: f32::INFINITY,
                    });
                }
                if let Some(item) = hovered_equipment {
                    let tooltip_key = format!(
                        "@status-equipment-tooltip-{}-{}-{}-{}-{}-{}-{}-{}-{}-{}",
                        crate::shop::hero_key(),
                        item.position,
                        item.item_type,
                        item.durability,
                        item.max_durability,
                        item.plus,
                        item.gem_one,
                        item.gem_two,
                        item.rarity,
                        format!("{:?}", item.affixes)
                    );
                    if !self.textures.contains_key(&tooltip_key) {
                        let info = crate::shop::item_info(assets, item.item_type);
                        self.upload(
                            &tooltip_key,
                            crate::shop::equipment_tooltip_image(
                                item.uid,
                                item.item_type,
                                info.as_ref(),
                                item.durability,
                                item.max_durability,
                                item.plus,
                                item.gem_one,
                                item.gem_two,
                                item.rarity,
                                item.affixes,
                                item.bless,
                                item.enchant_hp,
                            )?,
                        );
                    }
                    if let Some(texture) = self.textures.get(&tooltip_key) {
                        let mut position = [cursor[0] + 16.0, cursor[1] + 16.0];
                        if position[0] + texture.size[0] > viewport[0] {
                            position[0] = cursor[0] - texture.size[0] - 16.0;
                        }
                        if position[1] + texture.size[1] > viewport[1] {
                            position[1] = cursor[1] - texture.size[1] - 16.0;
                        }
                        draws.push(Draw {
                            texture: tooltip_key,
                            vertices: quad(position, texture.size, viewport),
                            depth_test: false,
                            sort_depth: f32::INFINITY,
                        });
                    }
                }
            }
        }
        if let Some((messages, options, avatar)) = dialog {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            messages.hash(&mut hasher);
            options.hash(&mut hasher);
            avatar.hash(&mut hasher);
            let hovered = crate::npc_dialog::hovered_option(viewport, cursor, messages, options);
            hovered.hash(&mut hasher);
            let close_hovered = crate::npc_dialog::close_hovered(viewport, cursor, messages, options);
            close_hovered.hash(&mut hasher);
            let key = format!("@npc-dialog-{}", hasher.finish());
            if !self.textures.contains_key(&key) {
                self.upload(
                    &key,
                    crate::npc_dialog::image(assets, messages, options, avatar, hovered, close_hovered)?,
                );
            }
            if let Some(texture) = self.textures.get(&key) {
                draws.push(Draw {
                    texture: key,
                    vertices: quad(crate::npc_dialog::position(viewport), texture.size, viewport),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
        }
        if inventory_open && shop.is_none() && warehouse.is_none() {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            inventory.hash(&mut hasher);
            silver.hash(&mut hasher);
            cps.hash(&mut hasher);
            inventory_filter.hash(&mut hasher);
            selected_inventory_uid.hash(&mut hasher);
            let hovered_filter = crate::shop::inventory_filter_at(viewport, cursor);
            hovered_filter.hash(&mut hasher);
            crate::shop::inventory_close_at(viewport, cursor).hash(&mut hasher);
            crate::shop::inventory_organize_at(viewport, cursor).hash(&mut hasher);
            let organize_hovered = crate::shop::inventory_organize_at(viewport, cursor);
            let signature = hasher.finish();
            let base_key = "@original-inventory-base".to_owned();
            if !self.textures.contains_key(&base_key) && !self.loading.contains(&base_key) {
                self.queue_ui_texture(
                    assets,
                    UiTextureJob::InventoryBase {
                        path: base_key.clone(),
                    },
                );
            }
            let key = format!("@inventory-overlay-{signature}");
            if self.inventory_signature != Some(signature) {
                self.queue_ui_texture(
                    assets,
                    UiTextureJob::InventoryOverlay {
                        path: key.clone(),
                        inventory: inventory.to_vec(),
                        silver,
                        cps,
                        selected_uid: selected_inventory_uid,
                        filter: inventory_filter,
                        hovered_filter,
                        close_hovered: crate::shop::inventory_close_at(viewport, cursor),
                        organize_hovered,
                    },
                );
                self.inventory_signature = Some(signature);
            }
            let inventory_base_draw_key = if self.textures.contains_key(&base_key) {
                self.last_inventory_key = Some(base_key.clone());
                Some(base_key.clone())
            } else {
                self.last_inventory_key.clone()
            };
            if let Some(inventory_base_draw_key) = inventory_base_draw_key
                && let Some(texture) = self.textures.get(&inventory_base_draw_key)
            {
                draws.push(Draw {
                    texture: inventory_base_draw_key,
                    vertices: quad(
                        crate::shop::inventory_position(viewport),
                        texture.size,
                        viewport,
                    ),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
            let lights = crate::shop::inventory_lights(inventory, inventory_filter);
            self.push_item_lights(assets, &lights, crate::shop::inventory_position(viewport), time, viewport, &mut draws);
            let inventory_overlay_draw_key = if self.textures.contains_key(&key) {
                self.last_inventory_overlay_key = Some(key.clone());
                Some(key.clone())
            } else {
                self.last_inventory_overlay_key.clone()
            };
            if let Some(inventory_overlay_draw_key) = inventory_overlay_draw_key
                && let Some(texture) = self.textures.get(&inventory_overlay_draw_key)
            {
                draws.push(Draw {
                    texture: inventory_overlay_draw_key,
                    vertices: quad(
                        crate::shop::inventory_position(viewport),
                        texture.size,
                        viewport,
                    ),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
        }
        if let Some(panel) = quick_panel
            && shop.is_none()
            && warehouse.is_none()
            && dialog.is_none()
        {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            panel.hash(&mut hasher);
            character.name.hash(&mut hasher);
            character.guild.hash(&mut hasher);
            character.guild_rank.hash(&mut hasher);
            character.guild_fund.hash(&mut hasher);
            action_bar.hash(&mut hasher);
            let mut skill_signature = known_skills.iter().collect::<Vec<_>>();
            skill_signature.sort_by_key(|(skill_id, _)| **skill_id);
            skill_signature.hash(&mut hasher);
            team_members.hash(&mut hasher);
            team_leader_id.hash(&mut hasher);
            pending_team_invite.hash(&mut hasher);
            pending_team_join_request.hash(&mut hasher);
            team_join_enabled.hash(&mut hasher);
            guild_members.hash(&mut hasher);
            pending_guild_invite.hash(&mut hasher);
            trade.active.hash(&mut hasher);
            trade.peer_id.hash(&mut hasher);
            trade.peer_name.hash(&mut hasher);
            trade.pending_request.hash(&mut hasher);
            trade.own_items.hash(&mut hasher);
            trade.peer_items.hash(&mut hasher);
            trade.own_silver.hash(&mut hasher);
            trade.peer_silver.hash(&mut hasher);
            trade.own_cps.hash(&mut hasher);
            trade.peer_cps.hash(&mut hasher);
            trade.silver_input.hash(&mut hasher);
            trade.cps_input.hash(&mut hasher);
            trade.editing.hash(&mut hasher);
            trade.own_confirmed.hash(&mut hasher);
            trade.peer_confirmed.hash(&mut hasher);
            booth.hash(&mut hasher);
            network_character_id.hash(&mut hasher);
            hud_config.hash(&mut hasher);
            if panel == crate::shop::QuickPanel::Quest {
                bounty.hash(&mut hasher);
            }
            if panel == crate::shop::QuickPanel::Nobility {
                nobility.hash(&mut hasher);
            }
            if panel == crate::shop::QuickPanel::Guild {
                guild.hash(&mut hasher);
            }
            if panel == crate::shop::QuickPanel::Highscores {
                highscores.hash(&mut hasher);
                // "Last update" reads in minutes.
                (crate::store_panel::now_secs() / 60).hash(&mut hasher);
            }
            if panel == crate::shop::QuickPanel::DailyRewards {
                daily.hash(&mut hasher);
                // The countdown to the next day reads in minutes.
                (crate::store_panel::now_secs() / 60).hash(&mut hasher);
            }
            if matches!(panel, crate::shop::QuickPanel::Store | crate::shop::QuickPanel::Vip) {
                store.hash(&mut hasher);
                // VIP time left reads in minutes.
                (web_time::SystemTime::now()
                    .duration_since(web_time::UNIX_EPOCH)
                    .map_or(0, |elapsed| elapsed.as_secs())
                    / 60)
                    .hash(&mut hasher);
                // The teleport countdown reads in seconds.
                if let Some(view) = store
                    && view.category == crate::store_panel::SERVICES
                    && crate::store_panel::teleport_wait(view, crate::store_panel::now_secs()) > 0
                {
                    crate::store_panel::now_secs().hash(&mut hasher);
                }
            }
            friends.hash(&mut hasher);
            ignores.hash(&mut hasher);
            pending_friend_name.hash(&mut hasher);
            selected_target.hash(&mut hasher);
            let selected_target_name = selected_target
                .and_then(|id| remote_actors.get(&id))
                .filter(|actor| !actor.npc && !(400001..=499999).contains(&actor.id))
                .map(|actor| actor.name.as_str())
                .or_else(|| {
                    selected_target.and_then(|id| {
                        team_members.iter().find(|member| member.member_id == id).map(|member| member.name.as_str())
                    })
                });
            selected_target_name.hash(&mut hasher);
            let signature = hasher.finish();
            let key = format!("@quick-panel-{signature}");
            if self.quick_panel_signature != Some(signature) {
                self.queue_ui_texture(
                    assets,
                    UiTextureJob::QuickPanel {
                        path: key.clone(),
                        panel,
                        character: character.clone(),
                        action_bar: action_bar.clone(),
                        known_skills: known_skills.clone(),
                        team_members: team_members.to_vec(),
                        team_leader_id,
                        pending_invite: pending_team_invite,
                        pending_join_request: pending_team_join_request,
                        team_join_enabled,
                        guild_members: guild_members.to_vec(),
                        pending_guild_invite,
                        friends: friends.to_vec(),
                        ignores: ignores.to_vec(),
                        pending_friend_name: pending_friend_name.map(str::to_owned),
                        selected_target_name: selected_target_name.map(str::to_owned),
                        inventory: inventory.to_vec(),
                        trade: trade.clone(),
                        booth: booth.clone(),
                        own_character_id: network_character_id,
                        hud_config,
                        bounty: bounty.cloned(),
                        store: store.cloned(),
                        daily: daily.cloned(),
                        highscores: highscores.cloned(),
                        nobility: nobility.cloned(),
                        guild: guild.cloned(),
                    },
                );
                self.quick_panel_signature = Some(signature);
            }
            if self.textures.contains_key(&key) {
                self.quick_panel_shown = Some((panel, key.clone()));
            }
            // Until the new look is ready, the last one of this panel.
            let shown = self
                .quick_panel_shown
                .as_ref()
                .filter(|(shown_panel, shown_key)| *shown_panel == panel && self.textures.contains_key(shown_key))
                .map(|(_, shown_key)| shown_key.clone());
            if let Some(texture) = shown.as_ref().and_then(|shown| self.textures.get(shown)) {
                draws.push(Draw {
                    texture: shown.clone().unwrap_or_default(),
                    vertices: quad(
                        crate::shop::quick_panel_position(viewport, panel),
                        texture.size,
                        viewport,
                    ),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
            // A booth's item under the cursor: its tooltip, with the price.
            if panel == crate::shop::QuickPanel::Market
                && let Some(listing) = crate::booth::item_at(viewport, cursor, booth, network_character_id)
            {
                let item = listing.item;
                let tooltip_key = format!(
                    "@booth-tooltip-{}-{}-{}-{}-{}-{}",
                    crate::shop::hero_key(),
                    item.uid,
                    item.item_type,
                    item.plus,
                    item.gem_one,
                    item.gem_two
                );
                if !self.textures.contains_key(&tooltip_key) {
                    let info = crate::shop::item_info(assets, item.item_type);
                    self.upload(
                        &tooltip_key,
                        crate::shop::item_tooltip_image(
                            item.uid,
                            item.item_type,
                            info.as_ref(),
                            item.plus,
                            item.gem_one,
                            item.gem_two,
                            item.rarity,
                            item.affixes,
                            item.bless,
                            item.enchant_hp,
                        )?,
                    );
                }
                if let Some(texture) = self.textures.get(&tooltip_key) {
                    let mut position = [cursor[0] + 14.0, cursor[1] + 14.0];
                    if position[0] + texture.size[0] > viewport[0] {
                        position[0] = cursor[0] - texture.size[0] - 14.0;
                    }
                    if position[1] + texture.size[1] > viewport[1] {
                        position[1] = cursor[1] - texture.size[1] - 14.0;
                    }
                    draws.push(Draw {
                        texture: tooltip_key,
                        vertices: quad(position, texture.size, viewport),
                        depth_test: false,
                        sort_depth: f32::INFINITY,
                    });
                }
            }
            // Beside the trade window: who the trade is with.
            if panel == crate::shop::QuickPanel::Trade
                && let Some(peer) = trade.peer_id.or(trade.pending_request)
            {
                let actor = remote_actors.get(&peer);
                let name = if trade.peer_name.is_empty() {
                    actor.map_or_else(|| peer.to_string(), |actor| actor.name.clone())
                } else {
                    trade.peer_name.clone()
                };
                let level = actor.map(|actor| actor.level).filter(|level| *level > 0);
                let key = format!("@trade-partner-{name}-{level:?}");
                if !self.textures.contains_key(&key) {
                    self.upload(
                        &key,
                        crate::original_panels::player_card(
                            assets,
                            &name,
                            level,
                            crate::original_panels::TRADE_PARTNER_INNER,
                        )?,
                    );
                }
                let [left, top] = crate::shop::quick_panel_position(viewport, panel);
                let position = [left + crate::original_panels::TRADE_WIDTH + 6.0, top];
                if let Some(texture) = self.textures.get(&key) {
                    draws.push(Draw {
                        texture: key,
                        vertices: quad(position, texture.size, viewport),
                        depth_test: false,
                        sort_depth: PREVIEW_SORT,
                    });
                }
                if let Some(model) = remote_models.get(&peer) {
                    let [top_center, size] = crate::original_panels::TRADE_PARTNER_MODEL;
                    self.push_character_preview(
                        assets,
                        model,
                        [position[0] + top_center[0], position[1] + top_center[1]],
                        size,
                        PREVIEW_SORT,
                        viewport,
                        &mut draws,
                    );
                }
            }
        }
        if let Some(shop) = shop {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            shop.npc_id.hash(&mut hasher);
            shop.items.hash(&mut hasher);
            shop.icons_generation.hash(&mut hasher);
            shop.page.hash(&mut hasher);
            shop.selected.hash(&mut hasher);
            shop.repair_hover.hash(&mut hasher);
            shop.sell_all_hover.hash(&mut hasher);
            shop.scroll_hover.hash(&mut hasher);
            let signature = hasher.finish();
            let base_key = "@original-shop-base".to_owned();
            if !self.textures.contains_key(&base_key) && !self.loading.contains(&base_key) {
                self.queue_ui_texture(
                    assets,
                    UiTextureJob::ShopBase {
                        path: base_key.clone(),
                    },
                );
            }
            let key = format!("@shop-overlay-{signature}");
            if self.shop_signature != Some(signature) {
                self.queue_ui_texture(
                    assets,
                    UiTextureJob::ShopOverlay {
                        path: key.clone(),
                        state: shop.clone(),
                    },
                );
                self.shop_signature = Some(signature);
            }
            let shop_base_draw_key = if self.textures.contains_key(&base_key) {
                self.last_shop_base_key = Some(base_key.clone());
                Some(base_key.clone())
            } else {
                self.last_shop_base_key.clone()
            };
            if let Some(shop_base_draw_key) = shop_base_draw_key
                && let Some(texture) = self.textures.get(&shop_base_draw_key)
            {
                draws.push(Draw {
                    texture: shop_base_draw_key,
                    vertices: quad(
                        crate::shop::panel_position(viewport),
                        texture.size,
                        viewport,
                    ),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
            let shop_overlay_draw_key = if self.textures.contains_key(&key) {
                self.last_shop_overlay_key = Some(key.clone());
                Some(key.clone())
            } else {
                self.last_shop_overlay_key.clone()
            };
            if let Some(shop_overlay_draw_key) = shop_overlay_draw_key
                && let Some(texture) = self.textures.get(&shop_overlay_draw_key)
            {
                draws.push(Draw {
                    texture: shop_overlay_draw_key,
                    vertices: quad(
                        crate::shop::panel_position(viewport),
                        texture.size,
                        viewport,
                    ),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
            if let Some(index) = shop.hover
                && shop.items.get(index).is_some()
                && shop.selected != Some(index)
            {
                if let Some(slot) = shop.visible_slot(index) {
                    let key = "@shop-hover";
                    if !self.textures.contains_key(key) {
                        self.upload(key, crate::shop::highlight_image());
                    }
                    if let Some(texture) = self.textures.get(key) {
                        draws.push(Draw {
                            texture: key.into(),
                            vertices: quad(
                                crate::shop::slot_position(viewport, slot),
                                texture.size,
                                viewport,
                            ),
                            depth_test: false,
                            sort_depth: f32::INFINITY,
                        });
                    }
                }
            }
            let mut inventory_hasher = std::collections::hash_map::DefaultHasher::new();
            inventory.hash(&mut inventory_hasher);
            silver.hash(&mut inventory_hasher);
            cps.hash(&mut inventory_hasher);
            inventory_filter.hash(&mut inventory_hasher);
            selected_inventory_uid.hash(&mut inventory_hasher);
            let hovered_filter = crate::shop::inventory_filter_at(viewport, cursor);
            hovered_filter.hash(&mut inventory_hasher);
            crate::shop::inventory_close_at(viewport, cursor).hash(&mut inventory_hasher);
            crate::shop::inventory_organize_at(viewport, cursor).hash(&mut inventory_hasher);
            let organize_hovered = crate::shop::inventory_organize_at(viewport, cursor);
            let inventory_signature = inventory_hasher.finish();
            let base_key = "@original-inventory-base".to_owned();
            if !self.textures.contains_key(&base_key) && !self.loading.contains(&base_key) {
                self.queue_ui_texture(
                    assets,
                    UiTextureJob::InventoryBase {
                        path: base_key.clone(),
                    },
                );
            }
            let inventory_key = format!("@inventory-overlay-{inventory_signature}");
            if self.inventory_signature != Some(inventory_signature) {
                self.queue_ui_texture(
                    assets,
                    UiTextureJob::InventoryOverlay {
                        path: inventory_key.clone(),
                        inventory: inventory.to_vec(),
                        silver,
                        cps,
                        selected_uid: selected_inventory_uid,
                        filter: inventory_filter,
                        hovered_filter,
                        close_hovered: crate::shop::inventory_close_at(viewport, cursor),
                        organize_hovered,
                    },
                );
                self.inventory_signature = Some(inventory_signature);
            }
            let inventory_base_draw_key = if self.textures.contains_key(&base_key) {
                self.last_inventory_key = Some(base_key.clone());
                Some(base_key.clone())
            } else {
                self.last_inventory_key.clone()
            };
            if let Some(inventory_base_draw_key) = inventory_base_draw_key
                && let Some(texture) = self.textures.get(&inventory_base_draw_key)
            {
                draws.push(Draw {
                    texture: inventory_base_draw_key,
                    vertices: quad(
                        crate::shop::inventory_position(viewport),
                        texture.size,
                        viewport,
                    ),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
            let lights = crate::shop::inventory_lights(inventory, inventory_filter);
            self.push_item_lights(assets, &lights, crate::shop::inventory_position(viewport), time, viewport, &mut draws);
            let inventory_overlay_draw_key = if self.textures.contains_key(&inventory_key) {
                self.last_inventory_overlay_key = Some(inventory_key.clone());
                Some(inventory_key.clone())
            } else {
                self.last_inventory_overlay_key.clone()
            };
            if let Some(inventory_overlay_draw_key) = inventory_overlay_draw_key
                && let Some(texture) = self.textures.get(&inventory_overlay_draw_key)
            {
                draws.push(Draw {
                    texture: inventory_overlay_draw_key,
                    vertices: quad(
                        crate::shop::inventory_position(viewport),
                        texture.size,
                        viewport,
                    ),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
            if let Some(index) = shop.hover.or(shop.selected) {
                if let Some(item_id) = shop.items.get(index) {
                    let tooltip_key = format!("@shop-tooltip-{item_id}-{}", crate::shop::hero_key());
                    if !self.textures.contains_key(&tooltip_key) {
                        self.upload(
                            &tooltip_key,
                            crate::shop::tooltip_image(*item_id, shop.item_info(*item_id))?,
                        );
                    }
                    if let Some(texture) = self.textures.get(&tooltip_key) {
                        let mut position = [cursor[0] + 14.0, cursor[1] + 14.0];
                        if position[0] + texture.size[0] > viewport[0] {
                            position[0] = cursor[0] - texture.size[0] - 14.0;
                        }
                        if position[1] + texture.size[1] > viewport[1] {
                            position[1] = cursor[1] - texture.size[1] - 14.0;
                        }
                        draws.push(Draw {
                            texture: tooltip_key,
                            vertices: quad(position, texture.size, viewport),
                            depth_test: false,
                            sort_depth: f32::INFINITY,
                        });
                    }
                }
            }
        }
        if let Some(warehouse) = warehouse {
            if shop.is_none() {
                use std::hash::{Hash, Hasher};
                let mut inventory_hasher = std::collections::hash_map::DefaultHasher::new();
                inventory.hash(&mut inventory_hasher);
                silver.hash(&mut inventory_hasher);
                cps.hash(&mut inventory_hasher);
                inventory_filter.hash(&mut inventory_hasher);
                selected_inventory_uid.hash(&mut inventory_hasher);
                let hovered_filter = crate::shop::inventory_filter_at(viewport, cursor);
                hovered_filter.hash(&mut inventory_hasher);
                crate::shop::inventory_close_at(viewport, cursor).hash(&mut inventory_hasher);
                crate::shop::inventory_organize_at(viewport, cursor).hash(&mut inventory_hasher);
                let organize_hovered = crate::shop::inventory_organize_at(viewport, cursor);
                let inventory_signature = inventory_hasher.finish();
                let base_key = "@original-inventory-base".to_owned();
                if !self.textures.contains_key(&base_key) && !self.loading.contains(&base_key) {
                    self.queue_ui_texture(
                        assets,
                        UiTextureJob::InventoryBase {
                            path: base_key.clone(),
                        },
                    );
                }
                let inventory_key = format!("@inventory-overlay-{inventory_signature}");
                if self.inventory_signature != Some(inventory_signature) {
                    self.queue_ui_texture(
                        assets,
                        UiTextureJob::InventoryOverlay {
                            path: inventory_key.clone(),
                            inventory: inventory.to_vec(),
                            silver,
                            cps,
                            selected_uid: selected_inventory_uid,
                            filter: inventory_filter,
                            hovered_filter,
                            close_hovered: crate::shop::inventory_close_at(viewport, cursor),
                            organize_hovered,
                        },
                    );
                    self.inventory_signature = Some(inventory_signature);
                }
                let inventory_base_draw_key = if self.textures.contains_key(&base_key) {
                    self.last_inventory_key = Some(base_key.clone());
                    Some(base_key.clone())
                } else {
                    self.last_inventory_key.clone()
                };
                if let Some(inventory_base_draw_key) = inventory_base_draw_key
                    && let Some(texture) = self.textures.get(&inventory_base_draw_key)
                {
                    draws.push(Draw {
                        texture: inventory_base_draw_key,
                        vertices: quad(
                            crate::shop::inventory_position(viewport),
                            texture.size,
                            viewport,
                        ),
                        depth_test: false,
                        sort_depth: f32::INFINITY,
                    });
                }
                let lights = crate::shop::inventory_lights(inventory, inventory_filter);
                self.push_item_lights(assets, &lights, crate::shop::inventory_position(viewport), time, viewport, &mut draws);
                let inventory_overlay_draw_key = if self.textures.contains_key(&inventory_key) {
                    self.last_inventory_overlay_key = Some(inventory_key.clone());
                    Some(inventory_key.clone())
                } else {
                    self.last_inventory_overlay_key.clone()
                };
                if let Some(inventory_overlay_draw_key) = inventory_overlay_draw_key
                    && let Some(texture) = self.textures.get(&inventory_overlay_draw_key)
                {
                    draws.push(Draw {
                        texture: inventory_overlay_draw_key,
                        vertices: quad(
                            crate::shop::inventory_position(viewport),
                            texture.size,
                            viewport,
                        ),
                        depth_test: false,
                        sort_depth: f32::INFINITY,
                    });
                }
            }
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            warehouse.hash(&mut hasher);
            guild_warehouse_open.hash(&mut hasher);
            warehouse_page.hash(&mut hasher);
            selected_warehouse_uid.hash(&mut hasher);
            warehouse_silver.hash(&mut hasher);
            warehouse_amount.hash(&mut hasher);
            warehouse_amount_focused.hash(&mut hasher);
            let signature = hasher.finish();
            let base_key = format!("@original-warehouse-base-{guild_warehouse_open}");
            if !self.textures.contains_key(&base_key) && !self.loading.contains(&base_key) {
                self.queue_ui_texture(
                    assets,
                    UiTextureJob::WarehouseBase {
                        path: base_key.clone(),
                        guild_warehouse: guild_warehouse_open,
                    },
                );
            }
            let key = format!("@warehouse-overlay-{signature}");
            if self.warehouse_signature != Some(signature) {
                self.queue_ui_texture(
                    assets,
                    UiTextureJob::WarehouseOverlay {
                        path: key.clone(),
                        items: warehouse.to_vec(),
                        page: warehouse_page,
                        selected_uid: selected_warehouse_uid,
                        saved_silver: warehouse_silver,
                        amount_text: warehouse_amount.to_owned(),
                        amount_focused: warehouse_amount_focused,
                        guild_warehouse: guild_warehouse_open,
                    },
                );
                self.warehouse_signature = Some(signature);
            }
            let warehouse_base_draw_key = if self.textures.contains_key(&base_key) {
                self.last_warehouse_base_key = Some(base_key.clone());
                Some(base_key.clone())
            } else {
                self.last_warehouse_base_key.clone()
            };
            if let Some(warehouse_base_draw_key) = warehouse_base_draw_key
                && let Some(texture) = self.textures.get(&warehouse_base_draw_key)
            {
                draws.push(Draw {
                    texture: warehouse_base_draw_key,
                    vertices: quad(
                        crate::shop::warehouse_position(viewport),
                        texture.size,
                        viewport,
                    ),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
            let warehouse_overlay_draw_key = if self.textures.contains_key(&key) {
                self.last_warehouse_overlay_key = Some(key.clone());
                Some(key.clone())
            } else {
                self.last_warehouse_overlay_key.clone()
            };
            if let Some(warehouse_overlay_draw_key) = warehouse_overlay_draw_key
                && let Some(texture) = self.textures.get(&warehouse_overlay_draw_key)
            {
                draws.push(Draw {
                    texture: warehouse_overlay_draw_key,
                    vertices: quad(
                        crate::shop::warehouse_position(viewport),
                        texture.size,
                        viewport,
                    ),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
        }
        // The depot's item under the cursor: its tooltip, as in the bag.
        if let Some(item) = warehouse
            .filter(|_| dragging_inventory_item.is_none())
            .and_then(|items| crate::shop::warehouse_item_at(items, viewport, cursor, warehouse_page))
        {
            let tooltip_key = format!(
                "@inventory-tooltip-{}-{}-{}-{}-{}-{}-{}-{:?}",
                crate::shop::hero_key(),
                item.uid,
                item.plus,
                item.gem_one,
                item.gem_two,
                item.item_type,
                item.rarity,
                item.affixes
            );
            if !self.textures.contains_key(&tooltip_key) {
                let info = crate::shop::item_info(assets, item.item_type);
                self.upload(
                    &tooltip_key,
                    crate::shop::item_tooltip_image(
                        item.uid,
                        item.item_type,
                        info.as_ref(),
                        item.plus,
                        item.gem_one,
                        item.gem_two,
                        item.rarity,
                        item.affixes,
                        item.bless,
                        item.enchant_hp,
                    )?,
                );
            }
            if let Some(texture) = self.textures.get(&tooltip_key) {
                let mut position = [cursor[0] + 14.0, cursor[1] + 14.0];
                if position[0] + texture.size[0] > viewport[0] {
                    position[0] = cursor[0] - texture.size[0] - 14.0;
                }
                if position[1] + texture.size[1] > viewport[1] {
                    position[1] = cursor[1] - texture.size[1] - 14.0;
                }
                draws.push(Draw {
                    texture: tooltip_key,
                    vertices: quad(position, texture.size, viewport),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
        }
        if (inventory_open || shop.is_some() || warehouse.is_some())
            && dragging_inventory_item.is_none()
            && let Some(item) =
                crate::shop::inventory_item_at(inventory, inventory_filter, viewport, cursor)
        {
            let hover_key = "@inventory-hover";
            if !self.textures.contains_key(hover_key) {
                self.upload(hover_key, crate::shop::highlight_image());
            }
            if let Some(texture) = self.textures.get(hover_key) {
                draws.push(Draw {
                    texture: hover_key.into(),
                    vertices: quad(
                        crate::shop::inventory_slot_position(viewport, item.position),
                        texture.size,
                        viewport,
                    ),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }

            let tooltip_key = format!(
                "@inventory-tooltip-{}-{}-{}-{}-{}-{}-{}-{}",
                crate::shop::hero_key(),
                item.uid,
                item.plus,
                item.gem_one,
                item.gem_two,
                item.item_type,
                item.rarity,
                format!("{:?}", item.affixes)
            );
            if !self.textures.contains_key(&tooltip_key) {
                let info = crate::shop::item_info(assets, item.item_type);
                self.upload(
                    &tooltip_key,
                    crate::shop::item_tooltip_image(
                        item.uid,
                        item.item_type,
                        info.as_ref(),
                        item.plus,
                        item.gem_one,
                        item.gem_two,
                        item.rarity,
                        item.affixes,
                        item.bless,
                        item.enchant_hp,
                    )?,
                );
            }
            if let Some(texture) = self.textures.get(&tooltip_key) {
                let mut position = [cursor[0] + 14.0, cursor[1] + 14.0];
                if position[0] + texture.size[0] > viewport[0] {
                    position[0] = cursor[0] - texture.size[0] - 14.0;
                }
                if position[1] + texture.size[1] > viewport[1] {
                    position[1] = cursor[1] - texture.size[1] - 14.0;
                }
                draws.push(Draw {
                    texture: tooltip_key,
                    vertices: quad(position, texture.size, viewport),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
        }
        if let Some(context) = item_context {
            let hovered = crate::shop::item_context_button_at(
                viewport,
                cursor,
                context.anchor,
                context.actions.len(),
            );
            let key = format!(
                "@item-context-{}-{:?}-{:?}",
                context.item.item_type, context.actions, hovered
            );
            if !self.textures.contains_key(&key) {
                let title = crate::shop::item_name(assets, context.item.item_type);
                self.upload(
                    &key,
                    crate::shop::item_context_image(
                        assets,
                        viewport,
                        context.anchor,
                        &title,
                        &context.actions,
                        hovered,
                    )?,
                );
            }
            if let Some(texture) = self.textures.get(&key) {
                let position = crate::shop::item_context_position(
                    viewport,
                    context.anchor,
                    context.actions.len(),
                );
                draws.push(Draw {
                    texture: key,
                    vertices: quad(position, texture.size, viewport),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
        }
        if let Some(menu) = choice_menu {
            let hovered = menu.row_at(cursor);
            let key = format!("@choice-menu-{:?}-{hovered:?}", (&menu.title, &menu.labels, menu.marked));
            if !self.textures.contains_key(&key) {
                self.upload(&key, menu.image(assets, hovered)?);
            }
            if let Some(texture) = self.textures.get(&key) {
                draws.push(Draw {
                    texture: key,
                    vertices: quad(menu.position.map(|value| value as f32), texture.size, viewport),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
        }
        if let Some((view, inventory, hovered)) = sell_all {
            let key = format!("@sell-all-{}", view.signature(inventory, hovered, view.at_shop));
            if !self.textures.contains_key(&key) {
                self.upload(&key, view.image(assets, inventory, hovered, view.at_shop)?);
            }
            if let Some(texture) = self.textures.get(&key) {
                draws.push(Draw {
                    texture: key,
                    vertices: quad(view.position.map(|value| value as f32), texture.size, viewport),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
        }
        if let Some(prompt) = prompt {
            let hovered = prompt.button_at(cursor);
            let key = format!("@prompt-{:?}-{hovered:?}", (prompt.title, &prompt.name, &prompt.text, prompt.buttons));
            if !self.textures.contains_key(&key) {
                self.upload(&key, prompt.image(assets, hovered)?);
            }
            if let Some(texture) = self.textures.get(&key) {
                draws.push(Draw {
                    texture: key,
                    vertices: quad(prompt.position.map(|value| value as f32), texture.size, viewport),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
        }
        // A city artisan's forge window.
        if let Some(view) = forge {
            let origin = crate::forge_window::position(viewport);
            let local = [cursor[0] - origin[0], cursor[1] - origin[1]];
            let hovered = crate::forge_window::covers(local)
                .then(|| crate::forge_window::action_at(equipment, local))
                .flatten();
            let flashing = view.last.is_some_and(|(_, at)| time - at < 2.0);
            let gear: Vec<_> = equipment.iter().map(|item| (item.uid, item.item_type, item.position, item.plus, item.durability)).collect();
            let catalysts = [crate::forge_rules::METEOR, crate::forge_rules::DRAGONBALL]
                .map(|catalyst| crate::forge_rules::count(inventory, catalyst, 0));
            let key = format!("@forge-{:?}-{hovered:?}-{flashing}-{gear:?}-{catalysts:?}", (view.selected, view.craft, view.busy, view.last.map(|(outcome, _)| outcome)));
            if !self.textures.contains_key(&key) {
                self.upload(&key, crate::forge_window::image(assets, &view, equipment, inventory, hovered, "ArtisanWind", time)?);
            }
            if let Some(texture) = self.textures.get(&key) {
                draws.push(Draw {
                    texture: key,
                    vertices: quad(origin, texture.size, viewport),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
        }
        // "Are you sure you want to exit?", over everything.
        if let Some(confirm) = exit_confirm {
            let hovered = if confirm.exercising { None } else { confirm.button_at(cursor) };
            let key = format!("@exit-confirm-{hovered:?}");
            if !self.textures.contains_key(&key) {
                self.upload(&key, confirm.image(assets, hovered)?);
            }
            if let Some(texture) = self.textures.get(&key) {
                draws.push(Draw {
                    texture: key,
                    vertices: quad(confirm.position.map(|value| value as f32), texture.size, viewport),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
        }
        if let Some(notice) = notice {
            let key = format!("@notice-{notice}");
            if !self.textures.contains_key(&key) {
                self.upload(&key, crate::shop::notice_image(notice)?);
            }
            if let Some(texture) = self.textures.get(&key) {
                let position = [((viewport[0] - texture.size[0] as f32) * 0.5).max(8.0), 48.0];
                draws.push(Draw {
                    texture: key,
                    vertices: quad(position, texture.size, viewport),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
        }
        if let Some(panel) = item_action_panel {
            let hovered = crate::shop::item_action_panel_button_at(viewport, cursor, panel.action);
            let close_hovered =
                crate::shop::item_action_panel_close_at(viewport, cursor, panel.action);
            let confirm_enabled = crate::shop::item_action_panel_confirm_enabled(panel, inventory);
            let catalyst_count = inventory.len();
            let key = format!(
                "@item-action-panel-{panel:?}-{hovered:?}-{confirm_enabled}-{close_hovered}-{catalyst_count}-anim{:02}",
                action_animation_progress
                    .map(|progress| (progress.clamp(0.0, 1.0) * 24.0).round() as u8)
                    .unwrap_or(255)
            );
            if !self.textures.contains_key(&key) {
                self.upload(
                    &key,
                    crate::shop::item_action_panel_image(
                        assets,
                        panel,
                        hovered,
                        inventory,
                        close_hovered,
                        action_animation_progress,
                    )?,
                );
            }
            if let Some(texture) = self.textures.get(&key) {
                draws.push(Draw {
                    texture: key,
                    vertices: quad(
                        crate::shop::item_action_panel_position(viewport, panel.action),
                        texture.size,
                        viewport,
                    ),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
        }
        if let Some(panel) = item_action_panel {
            let [left, top] = crate::shop::item_action_panel_position(viewport, panel.action);
            let local = [cursor[0] - left, cursor[1] - top];
            let item_hovered = match panel.action {
                crate::shop::ItemContextAction::Upgrade => {
                    (88.0..170.0).contains(&local[0]) && (74.0..154.0).contains(&local[1])
                        || (350.0..432.0).contains(&local[0]) && (74.0..154.0).contains(&local[1])
                }
                crate::shop::ItemContextAction::Rarity => {
                    (102.0..168.0).contains(&local[0]) && (68.0..138.0).contains(&local[1])
                        || (352.0..418.0).contains(&local[0]) && (68.0..138.0).contains(&local[1])
                }
                _ => (16.0..82.0).contains(&local[0]) && (47.0..113.0).contains(&local[1]),
            };
            if item_hovered {
                let tooltip_key = format!(
                    "@action-item-tooltip-{}-{}-{}-{}-{}-{}-{}-{}",
                    crate::shop::hero_key(),
                    panel.item.uid,
                    panel.item.plus,
                    panel.item.gem_one,
                    panel.item.gem_two,
                    panel.item.item_type,
                    panel.item.rarity,
                    format!("{:?}", panel.item.affixes)
                );
                if !self.textures.contains_key(&tooltip_key) {
                    let info = crate::shop::item_info(assets, panel.item.item_type);
                    self.upload(
                        &tooltip_key,
                        crate::shop::item_tooltip_image(
                            panel.item.uid,
                            panel.item.item_type,
                            info.as_ref(),
                            panel.item.plus,
                            panel.item.gem_one,
                            panel.item.gem_two,
                            panel.item.rarity,
                            panel.item.affixes,
                            panel.item.bless,
                            panel.item.enchant_hp,
                        )?,
                    );
                }
                if let Some(texture) = self.textures.get(&tooltip_key) {
                    let mut position = [cursor[0] + 16.0, cursor[1] + 16.0];
                    if position[0] + texture.size[0] > viewport[0] {
                        position[0] = cursor[0] - texture.size[0] - 16.0;
                    }
                    if position[1] + texture.size[1] > viewport[1] {
                        position[1] = cursor[1] - texture.size[1] - 16.0;
                    }
                    draws.push(Draw {
                        texture: tooltip_key,
                        vertices: quad(position, texture.size, viewport),
                        depth_test: false,
                        sort_depth: f32::INFINITY,
                    });
                }
            }
        }
        if let Some(panel) = gem_selection_panel {
            let hovered = crate::shop::gem_selection_panel_action_at(viewport, cursor, panel)
                .or_else(|| crate::shop::gem_selection_panel_button_at(viewport, cursor, inventory))
                .filter(|button| match button {
                    crate::shop::GemSelectionPanelButton::Socket(socket) => {
                        (*socket == 1 && panel.item.gem_one == 255)
                            || (*socket == 2 && panel.item.gem_two == 255)
                    }
                    crate::shop::GemSelectionPanelButton::Gem(_) => true,
                    crate::shop::GemSelectionPanelButton::OpenSocket(_)
                    | crate::shop::GemSelectionPanelButton::RemoveGem(_) => true,
                });
            let close_hovered = crate::shop::gem_selection_panel_close_at(viewport, cursor);
            let key = format!("@gem-selection-panel-{panel:?}-{hovered:?}-{close_hovered}");
            if !self.textures.contains_key(&key) {
                self.upload(
                    &key,
                    crate::shop::gem_selection_panel_image(
                        assets,
                        panel,
                        inventory,
                        hovered,
                        close_hovered,
                    )?,
                );
            }
            if let Some(texture) = self.textures.get(&key) {
                draws.push(Draw {
                    texture: key,
                    vertices: quad(
                        crate::shop::gem_selection_panel_position(viewport),
                        texture.size,
                        viewport,
                    ),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
        }
        if let (Some(item_id), Some(shop)) = (dragging_shop_item, shop) {
            let key = format!("@shop-drag-item-{item_id}");
            if !self.textures.contains_key(&key)
                && let Some(icon) = shop.icon_image(item_id).cloned()
            {
                self.upload(&key, icon);
            }
            if let Some(texture) = self.textures.get(&key) {
                let position = [
                    cursor[0] - texture.size[0] * 0.5,
                    cursor[1] - texture.size[1] * 0.5,
                ];
                draws.push(Draw {
                    texture: key,
                    vertices: quad(position, texture.size, viewport),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
        }
        if let Some(item) = dragging_inventory_item {
            let key = format!("@inventory-drag-item-{}", item.uid);
            if !self.textures.contains_key(&key) {
                self.upload(&key, crate::shop::item_icon_image(assets, item.item_type)?);
            }
            if let Some(texture) = self.textures.get(&key) {
                let position = [
                    cursor[0] - texture.size[0] * 0.5,
                    cursor[1] - texture.size[1] * 0.5,
                ];
                draws.push(Draw {
                    texture: key,
                    vertices: quad(position, texture.size, viewport),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
        }
        if performance_overlay {
            if self.perf_overlay_updated.elapsed() >= Duration::from_millis(250)
                || !self.textures.contains_key("@perf-overlay")
            {
                let average_ms = if self.perf_frames == 0 {
                    0.0
                } else {
                    self.perf_frame_time.as_secs_f64() * 1_000.0 / f64::from(self.perf_frames)
                };
                let texture_count = self.textures.len();
                self.upload(
                    "@perf-overlay",
                    crate::labels::performance_image(
                        average_ms,
                        self.perf_last_draws,
                        self.perf_last_vertices,
                        texture_count,
                    )?,
                );
                self.perf_overlay_updated = Instant::now();
            }
            if let Some(texture) = self.textures.get("@perf-overlay") {
                draws.push(Draw {
                    texture: "@perf-overlay".into(),
                    vertices: quad([8.0, 8.0], texture.size, viewport),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
        }
        // The chat log's line menu.
        if let Some((menu, hover)) = self.chat_menu.clone() {
            let key = format!("@chat-menu-{}-{}-{hover:?}", menu.sender, menu.at[0] * 10_000 + menu.at[1]);
            if !self.textures.contains_key(&key) {
                let image = crate::chat_log::menu_image(&menu, viewport, hover)?;
                self.prune_textures("@chat-menu-", &[]);
                self.upload(&key, image);
            }
            if let Some(texture) = self.textures.get(&key) {
                draws.push(Draw {
                    texture: key,
                    vertices: quad(crate::chat_log::menu_position(&menu, viewport), texture.size, viewport),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
        }
        // Private chats: the faces waiting over the bar and the window.
        for (index, (conversation, seen_mesh, hovered)) in skill_hud.whisper_faces.iter().enumerate() {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            std::hash::Hash::hash(&(&conversation.name, &conversation.card, seen_mesh, hovered), &mut hasher);
            let key = format!("@whisper-face-{}", std::hash::Hasher::finish(&hasher));
            if !self.textures.contains_key(&key) {
                let image = crate::whisper::face_icon(assets, conversation, *seen_mesh, *hovered)?;
                self.upload(&key, image);
            }
            if let Some(texture) = self.textures.get(&key) {
                let rect = crate::whisper::face_rect(viewport, index);
                draws.push(Draw {
                    texture: key,
                    vertices: quad([rect.x as f32, rect.y as f32], texture.size, viewport),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
        }
        if let Some(view) = &skill_hud.whisper_window {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            std::hash::Hash::hash(view, &mut hasher);
            let key = format!("@whisper-window-{}", std::hash::Hasher::finish(&hasher));
            if !self.textures.contains_key(&key) {
                let image = crate::whisper::window_image(
                    assets,
                    &view.conversation,
                    &view.own_name,
                    view.own_mesh,
                    view.seen_mesh,
                    &view.input,
                    view.input_focused,
                    view.hovered,
                )?;
                self.upload(&key, image);
            }
            if let Some(texture) = self.textures.get(&key) {
                draws.push(Draw {
                    texture: key,
                    vertices: quad(crate::whisper::window_position(viewport), texture.size, viewport),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
        }
        // The F keys' popup and a dragged emote go over every window.
        if let Some((view, hover)) = &skill_hud.hotkey_popup {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            std::hash::Hash::hash(&(view, hover), &mut hasher);
            let key = format!("@hotkey-popup-{}", std::hash::Hasher::finish(&hasher));
            if !self.textures.contains_key(&key) {
                let image = crate::hotkey_menu::image(assets, view, *hover)?;
                self.prune_textures("@hotkey-popup-", &[]);
                self.upload(&key, image);
            }
            if let Some(texture) = self.textures.get(&key) {
                draws.push(Draw {
                    texture: key,
                    vertices: quad(crate::hotkey_menu::position(view), texture.size, viewport),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
        }
        // The Travel Cube's window, over the other windows.
        if let Some(view) = self.travel_cube.clone() {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            view.hash(&mut hasher);
            let key = format!("@travel-cube-{}", hasher.finish());
            if !self.textures.contains_key(&key) {
                let font = crate::labels::font()?;
                let mut content = image::RgbaImage::new(crate::travel_cube::SIZE[0], crate::travel_cube::SIZE[1]);
                crate::travel_cube::draw(&mut content, &font, &view);
                let close = view.hover == Some(crate::travel_cube::CubeAction::Close);
                self.upload(&key, crate::shop::framed(assets, content, "Travel Cube", close)?);
            }
            if let Some(texture) = self.textures.get(&key) {
                draws.push(Draw {
                    texture: key,
                    vertices: quad(crate::travel_cube_position(viewport), texture.size, viewport),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
        }
        // The helper's window, over the other windows.
        if let Some(view) = self.helper.clone() {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            view.hash(&mut hasher);
            let key = format!("@helper-{}", hasher.finish());
            if !self.textures.contains_key(&key) {
                let font = crate::labels::font()?;
                let mut content = image::RgbaImage::new(crate::helper::SIZE[0], crate::helper::SIZE[1]);
                crate::helper::draw(&mut content, &font, &view, assets);
                let close = view.hover == Some(crate::helper::HelperAction::Close);
                self.upload(&key, crate::shop::framed(assets, content, "Helper", close)?);
            }
            if let Some(texture) = self.textures.get(&key) {
                draws.push(Draw {
                    texture: key,
                    vertices: quad(crate::helper_position(viewport), texture.size, viewport),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
        }
        // "While you were away", over the other windows.
        if let Some(view) = self.idle_report.clone() {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            view.hash(&mut hasher);
            let key = format!("@idle-report-{}", hasher.finish());
            if !self.textures.contains_key(&key) {
                let font = crate::labels::font()?;
                let mut content = image::RgbaImage::new(crate::idle_report::SIZE[0], crate::idle_report::SIZE[1]);
                crate::idle_report::draw(&mut content, &font, &view);
                self.upload(&key, crate::shop::framed(assets, content, "Helper", false)?);
            }
            if let Some(texture) = self.textures.get(&key) {
                draws.push(Draw {
                    texture: key,
                    vertices: quad(crate::idle_report_position(viewport), texture.size, viewport),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
        }
        // The Shortcuts window, over the other windows.
        if let Some(view) = self.shortcuts.clone() {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            view.hash(&mut hasher);
            let key = format!("@shortcuts-{}", hasher.finish());
            if !self.textures.contains_key(&key) {
                let font = crate::labels::font()?;
                let mut content = image::RgbaImage::new(crate::keybinds::SIZE[0], crate::keybinds::SIZE[1]);
                crate::keybinds::draw(&mut content, &font, &view);
                let close = view.hover == Some(crate::keybinds::ShortcutAction::Close);
                self.upload(&key, crate::shop::framed(assets, content, "Shortcuts", close)?);
            }
            if let Some(texture) = self.textures.get(&key) {
                draws.push(Draw {
                    texture: key,
                    vertices: quad(crate::keybinds_position(viewport), texture.size, viewport),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
        }
        if let Some(slot) = skill_hud.dragging_hotkey.and_then(|slot| action_bar.slots.get(slot)) {
            let key = format!("@hotkey-drag-{}", slot.action_id.as_deref().unwrap_or_default());
            if !self.textures.contains_key(&key) {
                let art = match slot.binding() {
                    Some(crate::action_bar::Binding::Item(item_type)) => crate::shop::item_icon_image(assets, item_type).ok(),
                    Some(crate::action_bar::Binding::Skill(id)) if slot.icon.is_none() => crate::main_hud::skill_icon(assets, id),
                    _ => slot
                        .icon_path()
                        .and_then(|path| assets.image(&path).ok())
                        .and_then(|art| crate::original_ui::visible_part(&art).ok()),
                };
                let art = art.map(|art| image::imageops::resize(&art, 34, 34, image::imageops::FilterType::Triangle));
                let art = art.unwrap_or_else(|| crate::labels::tooltip_image(&slot.hotkey).unwrap_or_default());
                self.upload(&key, art);
            }
            if let Some(texture) = self.textures.get(&key) {
                let position = [cursor[0] - texture.size[0] * 0.5, cursor[1] - texture.size[1] * 0.5];
                draws.push(Draw {
                    texture: key,
                    vertices: quad(position, texture.size, viewport),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
        }
        if let Some(index) = skill_hud.dragging_emote {
            let key = format!("@emote-drag-{index}");
            if !self.textures.contains_key(&key) {
                let art = assets.image(&format!("data/main/Act{index}.dds"))?;
                self.upload(&key, crate::original_ui::visible_part(&art)?);
            }
            if let Some(texture) = self.textures.get(&key) {
                let position = [cursor[0] - texture.size[0] * 0.5, cursor[1] - texture.size[1] * 0.5];
                draws.push(Draw {
                    texture: key,
                    vertices: quad(position, texture.size, viewport),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
            }
        }
        let ui_phase_finished = Instant::now();
        self.release_unused_transients(&draws);
        draws.sort_by(|a, b| a.sort_depth.total_cmp(&b.sort_depth));
        draws = coalesce_adjacent_draws(draws);
        let total_vertices = draws.iter().map(|draw| draw.vertices.len()).sum();
        self.perf_last_draws = draws.len();
        self.perf_last_vertices = total_vertices;
        let mut vertices = Vec::with_capacity(total_vertices);
        let mut ranges = Vec::with_capacity(draws.len());
        for draw in &draws {
            let start = vertices.len();
            vertices.extend_from_slice(&draw.vertices);
            ranges.push((start, vertices.len()));
        }
        let build_finished = Instant::now();
        self.write_vertices(&vertices);
        let upload_finished = Instant::now();
        if measure_perf {
            let build_time = build_finished.duration_since(frame_started);
            if build_time >= Duration::from_millis(8) {
                eprintln!(
                    "PERF_BUILD_BREAKDOWN total_ms={:.2} texture_phase_ms={:.2} geometry_phase_ms={:.2} minimap_world_ms={:.2} static_world_ms={:.2} remote_world_ms={:.2} ui_ms={:.2} finalize_ms={:.2}",
                    build_time.as_secs_f64() * 1_000.0,
                    texture_phase_finished
                        .duration_since(frame_started)
                        .as_secs_f64()
                        * 1_000.0,
                    geometry_phase_finished
                        .duration_since(texture_phase_finished)
                        .as_secs_f64()
                        * 1_000.0,
                    asset_phase_finished
                        .duration_since(geometry_phase_finished)
                        .as_secs_f64()
                        * 1_000.0,
                    static_world_phase_finished
                        .duration_since(asset_phase_finished)
                        .as_secs_f64()
                        * 1_000.0,
                    remote_world_phase_finished
                        .duration_since(static_world_phase_finished)
                        .as_secs_f64()
                        * 1_000.0,
                    ui_phase_finished
                        .duration_since(world_phase_finished)
                        .as_secs_f64()
                        * 1_000.0,
                    build_finished
                        .duration_since(ui_phase_finished)
                        .as_secs_f64()
                        * 1_000.0,
                );
            }
            self.perf_build_time += build_time;
            if build_time > self.perf_max_build_time {
                self.perf_max_build_time = build_time;
            }
            let upload_time = upload_finished.duration_since(build_finished);
            self.perf_upload_time += upload_time;
            if upload_time > self.perf_max_upload_time {
                self.perf_max_upload_time = upload_time;
            }
        }
        let acquire_started = Instant::now();
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("Surface validation failed".into());
            }
        };
        let acquire_finished = Instant::now();
        if measure_perf {
            let acquire_time = acquire_finished.duration_since(acquire_started);
            self.perf_acquire_time += acquire_time;
            if acquire_time > self.perf_max_acquire_time {
                self.perf_max_acquire_time = acquire_time;
            }
        }
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            // State changes only when it differs from the draw before.
            let mut current_clip: Option<bool> = None;
            let mut current_pipeline: Option<(bool, bool)> = None;
            let mut current_texture: Option<&str> = None;
            for (draw, (start, end)) in draws.iter().zip(&ranges) {
                let Some(texture) = self.textures.get(&draw.texture) else {
                    continue;
                };
                let clip = clipped_to_character(draw);
                if current_clip != Some(clip) {
                    if clip {
                        let [x, y, width, height] = crate::shop::status_character_clip([
                            self.config.width as f32,
                            self.config.height as f32,
                        ]);
                        pass.set_scissor_rect(x, y, width, height);
                    } else {
                        pass.set_scissor_rect(0, 0, self.config.width, self.config.height);
                    }
                    current_clip = Some(clip);
                }
                let glow = !draw.depth_test && draw.texture.ends_with(crate::magic::ADDITIVE_SUFFIX);
                if current_pipeline != Some((draw.depth_test, glow)) {
                    pass.set_pipeline(if draw.depth_test {
                        &self.actor
                    } else if glow {
                        &self.glow
                    } else {
                        &self.sprites
                    });
                    current_pipeline = Some((draw.depth_test, glow));
                }
                if current_texture != Some(draw.texture.as_str()) {
                    pass.set_bind_group(0, &texture.bind, &[]);
                    current_texture = Some(draw.texture.as_str());
                }
                let start = (*start * std::mem::size_of::<Vertex>()) as u64;
                let end = (*end * std::mem::size_of::<Vertex>()) as u64;
                pass.set_vertex_buffer(0, self.vertex_buffer.slice(start..end));
                pass.draw(0..draw.vertices.len() as u32, 0..1);
            }
        }
        let encode_finished = Instant::now();
        if measure_perf {
            let encode_time = encode_finished.duration_since(acquire_finished);
            self.perf_encode_time += encode_time;
            if encode_time > self.perf_max_encode_time {
                self.perf_max_encode_time = encode_time;
            }
        }
        let submit_started = Instant::now();
        self.queue.submit([encoder.finish()]);
        self.queue.present(frame);
        let submit_finished = Instant::now();
        if measure_perf {
            let submit_time = submit_finished.duration_since(submit_started);
            self.perf_submit_time += submit_time;
            if submit_time > self.perf_max_submit_time {
                self.perf_max_submit_time = submit_time;
            }
            self.perf_frames = self.perf_frames.saturating_add(1);
            let frame_time = frame_started.elapsed();
            self.perf_frame_time += frame_time;
            if frame_time > self.perf_max_frame_time {
                self.perf_max_frame_time = frame_time;
            }
            let report_elapsed = self.perf_started.elapsed();
            if report_elapsed >= Duration::from_secs(2) {
                let average_ms = self.perf_frame_time.as_secs_f64() * 1_000.0
                    / f64::from(self.perf_frames.max(1));
                let average = |duration: Duration| {
                    duration.as_secs_f64() * 1_000.0 / f64::from(self.perf_frames.max(1))
                };
                if self.perf_enabled {
                    let build_ms = average(self.perf_build_time);
                    let max_frame_ms = self.perf_max_frame_time.as_secs_f64() * 1_000.0;
                    let max_build_ms = self.perf_max_build_time.as_secs_f64() * 1_000.0;
                    let max_upload_ms = self.perf_max_upload_time.as_secs_f64() * 1_000.0;
                    let max_acquire_ms = self.perf_max_acquire_time.as_secs_f64() * 1_000.0;
                    let max_encode_ms = self.perf_max_encode_time.as_secs_f64() * 1_000.0;
                    let max_submit_ms = self.perf_max_submit_time.as_secs_f64() * 1_000.0;
                    let upload_ms = average(self.perf_upload_time);
                    let acquire_ms = average(self.perf_acquire_time);
                    let encode_ms = average(self.perf_encode_time);
                    let submit_ms = average(self.perf_submit_time);
                    eprintln!(
                        "PERF frames={} avg_frame_ms={average_ms:.2} max_frame_ms={max_frame_ms:.2} build_ms={build_ms:.2} max_build_ms={max_build_ms:.2} upload_ms={upload_ms:.2} max_upload_ms={max_upload_ms:.2} acquire_ms={acquire_ms:.2} max_acquire_ms={max_acquire_ms:.2} encode_ms={encode_ms:.2} max_encode_ms={max_encode_ms:.2} submit_ms={submit_ms:.2} max_submit_ms={max_submit_ms:.2} last_draws={} last_vertices={} textures={}",
                        self.perf_frames,
                        self.perf_last_draws,
                        self.perf_last_vertices,
                        self.textures.len(),
                    );
                }
                self.perf_started = Instant::now();
                self.perf_frames = 0;
                self.perf_frame_time = Duration::ZERO;
                self.perf_max_frame_time = Duration::ZERO;
                self.perf_build_time = Duration::ZERO;
                self.perf_max_build_time = Duration::ZERO;
                self.perf_upload_time = Duration::ZERO;
                self.perf_max_upload_time = Duration::ZERO;
                self.perf_acquire_time = Duration::ZERO;
                self.perf_max_acquire_time = Duration::ZERO;
                self.perf_encode_time = Duration::ZERO;
                self.perf_max_encode_time = Duration::ZERO;
                self.perf_submit_time = Duration::ZERO;
                self.perf_max_submit_time = Duration::ZERO;
            }
        }
        Ok(())
    }
}

/// Whether a draw is clipped to the status window's character view (the
/// paper doll), which the encoder sets as a scissor.
fn clipped_to_character(draw: &Draw) -> bool {
    draw.depth_test && draw.sort_depth == f32::INFINITY
}

/// How long a damage number shows (`Conquer.exe` 0x4d49cd: 250 ms coming
/// in, 1200 ms rising away).
pub const COMBAT_NUMBER_SECONDS: f32 = 1.45;

/// The colour a number rises in over its actor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberTint {
    /// A blow: yellow on the hero, white on anyone else.
    Blow,
    /// A potion's life.
    Life,
    /// A potion's mana.
    Mana,
}
/// The space each digit takes at full size, and a digit's size: the 64
/// pixel `CountB` sprites on their 92 by 122 unit quad, seen from 45
/// degrees and not shrunk as the world is (0.3 of it shows a 26 pixel
/// tall digit every 19 pixels, as the original).
const COUNT_SPACING: f32 = 64.0;
const COUNT_DIGIT: [f32; 2] = [92.0, 86.0];

/// How a damage number is drawn `elapsed` milliseconds in, as the original:
/// first it shrinks from full size to 0.3 as it comes in (alpha 32 to 255),
/// with an echo 1.3 times as large at 30%; then it rises 80 pixels in
/// 1200 ms, fading away. Each pass: scale, rise, alpha.
fn combat_number_passes(elapsed: f32) -> Vec<(f32, f32, f32)> {
    if elapsed < 0.0 {
        Vec::new()
    } else if elapsed <= 250.0 {
        let p = elapsed * 0.004;
        let scale = 1.0 - 0.7 * p;
        let alpha = (32.0 + 223.0 * p) / 255.0;
        vec![(scale * 1.3, 0.0, alpha * 0.3), (scale, 0.0, alpha)]
    } else if elapsed <= 1_450.0 {
        let p = (elapsed - 250.0) / 1_200.0;
        vec![(0.3, p * 80.0, 1.0 - p)]
    } else {
        Vec::new()
    }
}

/// How far an arrow is turned about the vertical to fly along `direction`
/// (on the screen): its model's tip points along -y, and the world is seen
/// from 45 degrees, its depth shortened by cos(45).
fn arrow_turn(direction: [f32; 2]) -> f32 {
    let cos = (-std::f32::consts::FRAC_PI_4).cos();
    let ground = [direction[0], -direction[1] / cos];
    ground[1].atan2(ground[0]) - (-1.0_f32).atan2(0.0)
}

/// Where an arrow flies, from the archer's and the target's ground points:
/// 25 pixels ahead of the archer, both 60 pixels up.
fn arrow_path(from: [f32; 2], to: [f32; 2]) -> ([f32; 2], [f32; 2]) {
    let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
    let length = dx.hypot(dy).max(1.0);
    (
        [from[0] + dx / length * ARROW_LEAD, from[1] + dy / length * ARROW_LEAD - ARROW_FROM_LIFT],
        [to[0], to[1] - ARROW_TO_LIFT],
    )
}

/// How long an arrow flies between two screen points.
fn arrow_seconds(start: [f32; 2], end: [f32; 2]) -> f32 {
    ((end[0] - start[0]).hypot(end[1] - start[1]) / ARROW_PIXELS_PER_SECOND).max(0.02)
}

/// When an arrow loosed now from `from` strikes `to` (ground points on the
/// screen): what it carries (the damage, the blow) lands then.
pub fn arrow_arrival(from: [f32; 2], to: [f32; 2]) -> f32 {
    let (start, end) = arrow_path(from, to);
    ARROW_RELEASE_SECONDS + arrow_seconds(start, end)
}

/// Merges neighbours of the sorted list that the GPU can draw in one call:
/// same texture, same pipeline and same clipping. Their order is kept, so
/// what overlaps what does not change; the depths only decided that order.
fn coalesce_adjacent_draws(draws: Vec<Draw>) -> Vec<Draw> {
    let mut merged: Vec<Draw> = Vec::with_capacity(draws.len());
    for draw in draws {
        if let Some(previous) = merged.last_mut()
            && previous.texture == draw.texture
            && previous.depth_test == draw.depth_test
            && clipped_to_character(previous) == clipped_to_character(&draw)
        {
            previous.vertices.extend(draw.vertices);
        } else {
            merged.push(draw);
        }
    }
    merged
}

/// The static sprites of `static_map` drawn with the camera at `camera`,
/// in their order, `extra` pixels more on every side. Queries only the
/// spatial buckets around the camera; the exact culling test still runs when
/// quads are built, so the bucket padding cannot change what is rendered.
fn static_indices_in_view(static_map: &StaticMapCache, camera: [f32; 2], viewport: [f32; 2], extra: f32) -> Vec<usize> {
    let max_margin = STATIC_OBJECT_MARGIN + extra;
    let min_cell = static_grid_cell([camera[0] - max_margin, camera[1] - max_margin]);
    let max_cell = static_grid_cell([camera[0] + viewport[0] + max_margin, camera[1] + viewport[1] + max_margin]);
    let mut candidates = Vec::new();
    for y in min_cell.1..=max_cell.1 {
        for x in min_cell.0..=max_cell.0 {
            if let Some(indices) = static_map.grid.get(&(x, y)) {
                candidates.extend(indices.iter().copied());
            }
        }
    }
    candidates.sort_unstable();
    candidates
        .into_iter()
        .filter(|index| {
            let sprite = &static_map.static_sprites[*index];
            let p = [sprite.origin[0] - camera[0], sprite.origin[1] - camera[1]];
            let margin = extra
                + if sprite.depth == f32::NEG_INFINITY {
                    STATIC_GROUND_MARGIN
                } else {
                    STATIC_OBJECT_MARGIN
                };
            p[0] <= viewport[0] + margin && p[1] <= viewport[1] + margin && p[0] >= -margin && p[1] >= -margin
        })
        .collect()
}

fn world_point_visible(origin: [f32; 2], viewport: [f32; 2], margin: f32) -> bool {
    origin[0] >= -margin
        && origin[1] >= -margin
        && origin[0] <= viewport[0] + margin
        && origin[1] <= viewport[1] + margin
}

/// Textures made for one moment and named after what they show; they are
/// recreated on demand, so the renderer may free them once unused (see
/// `release_unused_transients`). Images redrawn only when their signature
/// changes (inventory, status, panels) are not among them.
const TRANSIENT_PREFIXES: [&str; 23] = [
    "@exit-confirm-",
    "@travel-cube-",
    "@helper-",
    "@shortcuts-",
    "@booth-tooltip-",
    "@sell-all-",
    "@speech-",
    "@whisper-",
    "@actor-label-",
    "@monster-label-",
    "@label-",
    "@damage-",
    "@loot-name-",
    "@minimap-pin-name-",
    "@choice-menu-",
    "@prompt-",
    "@forge-",
    "@notice-",
    "@shop-tooltip-",
    "@trade-partner-",
    "@item-context-",
    "@inventory-drag-item-",
    "@shop-drag-item-",
];
/// About ten seconds at 60 FPS.
const TRANSIENT_FRAMES: u64 = 600;

fn is_transient(key: &str) -> bool {
    key.starts_with('@') && TRANSIENT_PREFIXES.iter().any(|prefix| key.starts_with(prefix))
}

/// How long a feedback message stays at the top of the screen.
const FEEDBACK_SECONDS: f32 = 4.0;

/// `GRAVAK_DEBUG_STATUS=1` keeps every client status on screen, the
/// `online: ...` network trace included.
fn debug_status() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("GRAVAK_DEBUG_STATUS")
            .is_ok_and(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
    })
}

/// Whether a client status is shown at the top of the screen: player-facing
/// notices are, the network trace ("online: ...", "loading ...") only when
/// debugging.
fn show_feedback(status: &str) -> bool {
    let status = status.trim();
    if status.is_empty() {
        return false;
    }
    debug_status()
        || !["online:", "online ", "loading", "connecting", "Network"]
            .iter()
            .any(|prefix| status.starts_with(prefix))
}

/// A point of an effect turned by its wielder's facing, as the character's
/// own vertices are (`Character::project`).
/// Where a soul's flight is `way` along (0 over the monster slain at `from`,
/// where it rose to, 1 over its slayer at `to`; both on screen): a curve to
/// one side, which side and how far its own (by where it fell), and a
/// little up, 40 pixels over the ground at its end.
fn soul_path(soul: Soul, from: [f32; 2], to: [f32; 2], way: f32) -> [f32; 2] {
    let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
    let length = dx.hypot(dy).max(1.0);
    let seed = ((soul.from[0] * 12.9898 + soul.from[1] * 78.233).sin() * 43_758.547).rem_euclid(1.0);
    let bend = if seed >= 0.5 { 1.0 } else { -1.0 } * (0.12 + 0.22 * seed) * length.min(400.0);
    let swing = (way * std::f32::consts::PI).sin();
    [
        from[0] + dx * way - dy / length * bend * swing,
        from[1] + dy * way + dx / length * bend * swing - 40.0 - 30.0 * (1.0 - way) - swing * 30.0,
    ]
}

/// A soul's trail across its width: white at the middle, its tint toward
/// the edges, fading out to them.
fn soul_trail_image(tint: [f32; 3]) -> image::RgbaImage {
    image::RgbaImage::from_fn(4, 32, |_, y| {
        let across = ((y as f32 + 0.5) / 32.0 * 2.0 - 1.0).abs();
        let light = (1.0 - across).max(0.0).powf(1.5);
        let heart = (1.0 - across * 3.0).max(0.0);
        let colour = |channel: usize| ((tint[channel] * (1.0 - heart) + heart) * light * 255.0) as u8;
        image::Rgba([colour(0), colour(1), colour(2), (light * 255.0) as u8])
    })
}

/// The colour a quality's light glows on the ground.
fn ground_glow_colour(light: &str) -> [f32; 3] {
    match light {
        "FireLight" => [1.0, 0.72, 0.3],
        "RedLight" => [0.95, 0.3, 0.28],
        "BlueLight" => [0.35, 0.75, 1.0],
        "RoyalBlueLight" => [0.35, 0.45, 1.0],
        "GoldEnLight" => [1.0, 0.85, 0.4],
        _ => [1.0, 0.92, 0.45],
    }
}

/// A glow lying on the ground: an ellipse twice as wide as tall, `colour`
/// at its heart fading to nothing at its edge, for drawing additively.
fn ground_glow_image(colour: [f32; 3]) -> image::RgbaImage {
    const WIDTH: u32 = 128;
    const HEIGHT: u32 = 64;
    image::RgbaImage::from_fn(WIDTH, HEIGHT, |x, y| {
        let dx = (x as f32 + 0.5) / WIDTH as f32 * 2.0 - 1.0;
        let dy = (y as f32 + 0.5) / HEIGHT as f32 * 2.0 - 1.0;
        let light = (1.0 - dx.hypot(dy)).max(0.0).powf(1.8);
        let channel = |c: f32| (c * light * 255.0) as u8;
        image::Rgba([channel(colour[0]), channel(colour[1]), channel(colour[2]), (light * 255.0) as u8])
    })
}

/// A sparkle: a four pointed star with a soft glow, white at its heart and
/// `tint` (per channel) toward its edge, for drawing additively.
fn sparkle_image(tint: [f32; 3]) -> image::RgbaImage {
    const SIZE: u32 = 32;
    image::RgbaImage::from_fn(SIZE, SIZE, |x, y| {
        let (dx, dy) = ((x as f32 + 0.5) / SIZE as f32 * 2.0 - 1.0, (y as f32 + 0.5) / SIZE as f32 * 2.0 - 1.0);
        let r = dx.hypot(dy);
        let rays = (1.0 - (dx.abs() * dy.abs()).sqrt() * 9.0).max(0.0) * (1.0 - r).max(0.0);
        let glow = (1.0 - r).max(0.0).powi(3) * 0.8;
        let light = (rays + glow).min(1.0);
        let heart = (1.0 - r * 2.5).max(0.0);
        let colour = |channel: usize| ((tint[channel] * (1.0 - heart) + heart) * light * 255.0) as u8;
        image::Rgba([colour(0), colour(1), colour(2), (light * 255.0) as u8])
    })
}

fn turned(p: sandbox_data::c3::Vector3, yaw: Option<f32>) -> sandbox_data::c3::Vector3 {
    let Some(yaw) = yaw else {
        return p;
    };
    let (s, c) = yaw.sin_cos();
    sandbox_data::c3::Vector3 { x: p.x * c - p.y * s, y: p.x * s + p.y * c, z: p.z }
}

/// Client-side remote action codes: a dead player (the 401 fall, its last
/// frame held) and a dead monster fading out. The rest are the server's
/// ChangeAction codes; sitting, kneeling and lying hold their last frame.
pub(crate) const REMOTE_DEAD: u16 = 1161;
/// The colour of NPCs' names over their heads.
const NPC_NAME: [u8; 3] = [96, 236, 96];
pub(crate) const REMOTE_FADING: u16 = 1160;

/// `wanted` kept so the view stays on the map; a map smaller than the view
/// sits in its middle.
/// How far the world zooms out and in: a tenth of the way per notch of the
/// wheel. Past 0.8 the screen would show more than the server tells of
/// (`VIEW_DISTANCE`, 34 cells about the hero) on a large window.
pub const ZOOM_RANGE: std::ops::RangeInclusive<f32> = 0.8..=2.0;

fn clamp_camera(wanted: [f32; 2], extent: [f32; 2], view: [f32; 2]) -> [f32; 2] {
    [0, 1].map(|axis| {
        let room = extent[axis] - view[axis];
        if room <= 0.0 { room * 0.5 } else { wanted[axis].clamp(0.0, room) }
    })
}

fn remote_action(value: u16) -> Action {
    match value {
        REMOTE_DEAD => Action::Die,
        _ => Action::from_catalog_id(u32::from(value)).unwrap_or(Action::Idle),
    }
}

/// Whether a remote action's last frame stays once it has played.
fn remote_held(value: u16) -> bool {
    value == REMOTE_DEAD || remote_action(value).held()
}

fn should_hide_dead_monster(
    health: u16,
    action: u16,
    animation_until: f32,
    respawn_until: f32,
    time: f32,
) -> bool {
    health == 0 && action == REMOTE_FADING && time >= animation_until && time >= respawn_until
}

fn clip(p: [f32; 2], z: f32, viewport: [f32; 2]) -> [f32; 3] {
    [
        p[0] / viewport[0] * 2.0 - 1.0,
        1.0 - p[1] / viewport[1] * 2.0,
        z,
    ]
}
/// C3 draw keys switch a mesh on and off over time; no keys means visible.
fn draw_key_visible(keys: &[sandbox_data::c3::DrawKey], frame: f32) -> bool {
    keys.iter()
        .take_while(|key| key.frame as f32 <= frame)
        .last()
        .or(keys.first())
        .is_none_or(|key| key.visible)
}

fn transparency_at(keys: &[TransparencyKey], frame: f32) -> f32 {
    let Some(first) = keys.first() else {
        return 1.0;
    };
    if frame <= first.frame as f32 {
        return first.alpha.clamp(0.0, 1.0);
    }
    for pair in keys.windows(2) {
        let left = pair[0];
        let right = pair[1];
        if frame <= right.frame as f32 {
            let span = (right.frame - left.frame).max(1) as f32;
            let amount = ((frame - left.frame as f32) / span).clamp(0.0, 1.0);
            return (left.alpha + (right.alpha - left.alpha) * amount).clamp(0.0, 1.0);
        }
    }
    keys.last().map_or(1.0, |key| key.alpha.clamp(0.0, 1.0))
}
fn quad(p: [f32; 2], size: [f32; 2], viewport: [f32; 2]) -> Vec<Vertex> {
    quad_with_alpha(p, size, viewport, 1.0)
}
fn quad_with_alpha(p: [f32; 2], size: [f32; 2], viewport: [f32; 2], alpha: f32) -> Vec<Vertex> {
    let points = [
        ([p[0], p[1]], [0.0, 0.0]),
        ([p[0] + size[0], p[1]], [1.0, 0.0]),
        ([p[0] + size[0], p[1] + size[1]], [1.0, 1.0]),
        ([p[0], p[1] + size[1]], [0.0, 1.0]),
    ];
    [0, 1, 2, 0, 2, 3]
        .map(|i| Vertex {
            position: clip(points[i].0, 0.9, viewport),
            uv: points[i].1,
            alpha: alpha.clamp(0.0, 1.0),
        })
        .to_vec()
}
/// A player's face portrait from `data/PlayerFace/JPG/64`, scaled to `size`
/// pixels square. The face is the avatar part of the look
/// (`look = body + avatar * 10000 + transformation * 10000000`).
pub(crate) fn player_face(
    assets: &mut crate::assets::Assets,
    mesh: u32,
    size: u32,
) -> Option<image::RgbaImage> {
    let face = mesh / 10_000 % 1_000;
    if face == 0 {
        return None;
    }
    let mut paths = player_face_ani_paths(assets, face);
    for extension in ["dds", "jpg", "DDS", "JPG"] {
        paths.push(format!("data/PlayerFace/JPG/64/{face}.{extension}"));
        paths.push(format!("data/PlayerFace/JPG/64/{face:03}.{extension}"));
    }
    let image = paths.iter().find_map(|path| assets.image(path).ok())?;
    Some(image::imageops::resize(
        &image,
        size,
        size,
        image::imageops::FilterType::Triangle,
    ))
}

/// The 64px frames `ani/PlayerFace.ani` binds to `face`: sections whose
/// name, or whose frame's file name, carries the face number.
fn player_face_ani_paths(assets: &mut crate::assets::Assets, face: u32) -> Vec<String> {
    let Ok(bytes) = assets.read("ani/PlayerFace.ani") else {
        return Vec::new();
    };
    let number = |value: &str| {
        let digits: String = value.chars().filter(char::is_ascii_digit).collect();
        digits.parse::<u32>().ok()
    };
    let mut section = String::new();
    let mut paths = Vec::new();
    for line in String::from_utf8_lossy(&bytes).lines().map(str::trim) {
        if let Some(name) = line.strip_prefix('[').and_then(|line| line.strip_suffix(']')) {
            section = name.to_owned();
            continue;
        }
        let Some((key, path)) = line.split_once('=') else {
            continue;
        };
        let path = path.trim().replace('\\', "/");
        if !key.trim().eq_ignore_ascii_case("Frame0") || !path.to_ascii_lowercase().contains("/64/") {
            continue;
        }
        let stem = path.rsplit('/').next().unwrap_or("").split('.').next().unwrap_or("");
        if number(&section) == Some(face) || number(stem) == Some(face) {
            paths.push(path);
        }
    }
    paths
}

#[cfg(test)]
mod effect_sheet;

#[cfg(test)]
mod player_face_probe {
    /// Prints how `ani/PlayerFace.ani` names the faces and which portrait
    /// each of the first faces resolves to, against a local 5065 install.
    #[test]
    #[ignore = "Needs the 5065 install (GRAVAK_CLIENT, default R:/5065)"]
    fn probe_player_faces() -> crate::assets::Result<()> {
        let root = std::env::var("GRAVAK_CLIENT").unwrap_or_else(|_| "R:/5065".into());
        let mut assets = crate::assets::Assets::open(std::path::Path::new(&root))?;
        match assets.read("ani/PlayerFace.ani") {
            Ok(bytes) => {
                for line in String::from_utf8_lossy(&bytes).lines().take(40) {
                    println!("ani: {line}");
                }
            }
            Err(error) => println!("ani/PlayerFace.ani: {error}"),
        }
        for face in (1..=12).chain(201..=212) {
            let paths = super::player_face_ani_paths(&mut assets, face);
            let found = super::player_face(&mut assets, face * 10_000 + 1003, crate::labels::PLAYER_FACE_SIZE)
                .is_some();
            println!("face {face}: ani {paths:?}, loaded {found}");
        }
        Ok(())
    }
}

fn depth(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

/// How high a remote actor's jump (action 130) lifts them now, along the
/// local player's arc.
fn remote_jump_lift(actor: &RemoteActor, time: f32) -> f32 {
    // A flier glides level.
    if actor.status_flags & crate::network::FLY_FLAG != 0 {
        return 0.0;
    }
    if !(actor.action == 130 || actor.jumping)
        || actor.movement_until <= actor.movement_started
        || time >= actor.movement_until
    {
        return 0.0;
    }
    let progress = (time - actor.movement_started) / (actor.movement_until - actor.movement_started);
    let peak =
        crate::motion_math::jump_motion(actor.movement_from, actor.movement_to, std::time::Duration::ZERO)
            .peak_screen_pixels;
    crate::motion_math::jump_screen_height(peak, progress)
}

/// The sprite-sheet cell a mesh shows at `frame`, as a UV offset: its
/// texture keys pick cells of a grid the size of its UVs.
fn sheet_cell(mesh: &sandbox_data::c3::C3Mesh, frame: f32) -> [f32; 2] {
    let Some(key) = mesh
        .texture_keys
        .iter()
        .rev()
        .find(|key| key.frame as f32 <= frame)
        .or(mesh.texture_keys.first())
    else {
        return [0.0, 0.0];
    };
    let span = mesh.vertices.iter().fold([0.0_f32; 2], |span, vertex| {
        [span[0].max(vertex.texture_coordinate.x), span[1].max(vertex.texture_coordinate.y)]
    });
    if span[0] <= 0.0 || span[0] >= 1.0 || span[1] <= 0.0 {
        return [0.0, 0.0];
    }
    let columns = (1.0 / span[0]).round().max(1.0) as i32;
    let cell = key.texture.max(0);
    [(cell % columns) as f32 * span[0], (cell / columns) as f32 * span[1]]
}

/// Where a trail (newest first) passed at `when`, between its samples;
/// `None` when it holds nothing that old or new.
fn trail_point(trail: &[(f32, [f32; 2])], when: f32) -> Option<[f32; 2]> {
    let (newest_time, newest) = *trail.first()?;
    if when >= newest_time {
        return Some(newest);
    }
    for pair in trail.windows(2) {
        let ((later_time, later), (earlier_time, earlier)) = (pair[0], pair[1]);
        if when >= earlier_time {
            let span = (later_time - earlier_time).max(f32::EPSILON);
            let blend = (when - earlier_time) / span;
            return Some([
                earlier[0] + (later[0] - earlier[0]) * blend,
                earlier[1] + (later[1] - earlier[1]) * blend,
            ]);
        }
    }
    trail.last().map(|(_, oldest)| *oldest)
}

#[cfg(test)]
mod tests {
    use super::{
        Draw, clamp_camera, coalesce_adjacent_draws, quad, should_hide_dead_monster, world_point_visible,
    };

    #[test]
    fn the_view_stays_on_the_map() {
        let extent = [26_368.0, 17_920.0];
        let view = [1024.0, 768.0];
        // In the middle of the map: centred as wanted.
        assert_eq!(clamp_camera([5_000.0, 6_000.0], extent, view), [5_000.0, 6_000.0]);
        // Near the top (Twin City's Bird Island portal): no black above.
        assert_eq!(clamp_camera([13_856.0, -224.0], extent, view), [13_856.0, 0.0]);
        // Near the bottom right corner.
        assert_eq!(clamp_camera([26_000.0, 17_500.0], extent, view), [25_344.0, 17_152.0]);
        // A map smaller than the view sits in its middle.
        assert_eq!(clamp_camera([10.0, 10.0], [800.0, 600.0], view), [-112.0, -84.0]);
    }

    #[test]
    fn neighbours_of_one_texture_draw_in_one_call() {
        let draw = |texture: &str, depth_test: bool, sort_depth: f32| Draw {
            texture: texture.to_owned(),
            vertices: quad([0.0, 0.0], [8.0, 8.0], [100.0, 100.0]),
            depth_test,
            sort_depth,
        };
        let merged = coalesce_adjacent_draws(vec![
            draw("tiles", false, 1.0),
            draw("tiles", false, 1.5),
            draw("tiles", false, 2.0),
            draw("hero", true, 2.5),
            draw("tiles", false, 3.0),
            // The paper doll keeps its own call: it is clipped.
            draw("hero", true, f32::INFINITY),
            draw("hero", true, f32::INFINITY),
        ]);
        let shape: Vec<(&str, usize)> = merged
            .iter()
            .map(|draw| (draw.texture.as_str(), draw.vertices.len() / 6))
            .collect();
        assert_eq!(shape, [("tiles", 3), ("hero", 1), ("tiles", 1), ("hero", 2)]);
    }

    #[test]
    fn sprite_quad_keeps_its_winding_and_uvs() {
        let vertices = quad([0.0, 0.0], [100.0, 50.0], [200.0, 100.0]);
        assert_eq!(vertices.len(), 6);
        assert_eq!(
            vertices.iter().map(|v| v.position).collect::<Vec<_>>(),
            [
                [-1.0, 1.0, 0.9],
                [0.0, 1.0, 0.9],
                [0.0, 0.0, 0.9],
                [-1.0, 1.0, 0.9],
                [0.0, 0.0, 0.9],
                [-1.0, 0.0, 0.9],
            ]
        );
        assert_eq!(
            vertices.iter().map(|v| v.uv).collect::<Vec<_>>(),
            [
                [0.0, 0.0],
                [1.0, 0.0],
                [1.0, 1.0],
                [0.0, 0.0],
                [1.0, 1.0],
                [0.0, 1.0],
            ]
        );
    }

    #[test]
    fn dead_monster_is_hidden_after_death_animation_until_respawn() {
        assert!(!should_hide_dead_monster(0, super::REMOTE_FADING, 1.2, 0.0, 1.19));
        assert!(should_hide_dead_monster(0, super::REMOTE_FADING, 1.2, 0.0, 1.2));
        assert!(!should_hide_dead_monster(100, super::REMOTE_FADING, 1.2, 0.0, 2.0));
        assert!(!should_hide_dead_monster(0, super::REMOTE_FADING, 1.2, 3.0, 2.0));
    }

    #[test]
    fn adjacent_draws_share_a_texture_without_reordering_layers() {
        let vertices = quad([0.0, 0.0], [10.0, 10.0], [20.0, 20.0]);
        let make = |texture: &str, depth_test: bool, sort_depth: f32| Draw {
            texture: texture.to_owned(),
            vertices: vertices.clone(),
            depth_test,
            sort_depth,
        };
        let merged = coalesce_adjacent_draws(vec![
            make("same", false, 1.0),
            make("same", false, 1.0),
            make("different", false, 1.0),
            make("same", false, 1.0),
            make("same", true, 1.0),
        ]);
        assert_eq!(merged.len(), 4);
        assert_eq!(merged[0].vertices.len(), 12);
        assert_eq!(merged[1].texture, "different");
        assert_eq!(merged[2].vertices.len(), 6);
        assert!(merged[3].depth_test);
    }

    #[test]
    fn world_culling_keeps_edge_margin_but_rejects_far_entities() {
        let viewport = [1_000.0, 700.0];
        assert!(world_point_visible([-96.0, 350.0], viewport, 96.0));
        assert!(world_point_visible([1_096.0, 350.0], viewport, 96.0));
        assert!(!world_point_visible([-96.1, 350.0], viewport, 96.0));
        assert!(!world_point_visible([500.0, 900.1], viewport, 96.0));
    }
}

#[cfg(test)]
mod transient_tests {
    #[test]
    fn only_on_demand_textures_are_released() {
        for key in ["@monster-label-400010-37", "@label-Gravak-80-120-[255, 255, 255]", "@damage-128", "@item-context-410003-[Equip]-None"] {
            assert!(super::is_transient(key), "{key}");
        }
        // Redrawn only when their signature changes: never released.
        for key in ["@hud-123", "@inventory-overlay-9", "@quick-panel-4", "@shadow", "c3/texture/1.dds"] {
            assert!(!super::is_transient(key), "{key}");
        }
    }
}



/// Shift Screen's step (`Conquer.exe` 0x481933, called each frame by
/// 0x469676 with Alt held and the box ticked): within 120 pixels of an
/// edge the view moves toward it by up to 6 pixels a frame, more the
/// nearer the cursor (the bottom's band sits 50 pixels higher, over the
/// bar); it goes at most 356 pixels aside, 172 up and 192 down. `frames`
/// is the time passed in the original's frames.
pub fn shift_screen_step(shift: [f32; 2], cursor: [f32; 2], viewport: [f32; 2], frames: f32) -> [f32; 2] {
    let [x, y] = cursor;
    let [width, height] = viewport;
    let mut step = [0.0, 0.0];
    if x <= 120.0 {
        step[0] += (x / 20.0).trunc() - 6.0;
    }
    if x >= width - 120.0 {
        step[0] += 6.0 - ((width - x) / 20.0).trunc();
    }
    if y <= 120.0 {
        step[1] += (y / 20.0).trunc() - 6.0;
    }
    if y >= height - 120.0 {
        step[1] += 6.0 - ((height - 50.0 - y) / 20.0).trunc();
    }
    [
        (shift[0] + step[0] * frames).clamp(-356.0, 356.0),
        (shift[1] + step[1] * frames).clamp(-172.0, 192.0),
    ]
}

#[cfg(test)]
mod shift_screen_tests {
    use super::shift_screen_step;

    #[test]
    fn the_view_moves_toward_the_edge_under_the_cursor() {
        let viewport = [1024.0, 768.0];
        assert_eq!(shift_screen_step([0.0, 0.0], [512.0, 384.0], viewport, 1.0), [0.0, 0.0]);
        assert_eq!(shift_screen_step([0.0, 0.0], [0.0, 384.0], viewport, 1.0), [-6.0, 0.0]);
        assert_eq!(shift_screen_step([0.0, 0.0], [1023.0, 0.0], viewport, 1.0), [6.0, -6.0]);
        assert_eq!(shift_screen_step([350.0, 190.0], [1023.0, 767.0], viewport, 10.0), [356.0, 192.0]);
    }
}
