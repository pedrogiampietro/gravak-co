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
    time::{Duration, Instant},
};
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
/// Sort depth of what floats over the world (names, nameplates, damage
/// numbers, ground loot labels): above every world draw but below the UI,
/// the trade partner card included, so a name never shows through a window.
const WORLD_OVERLAY_SORT: f32 = f32::MAX / 2.0;

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
}

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
const STATUS_EFFECTS: [(u64, &str); 7] = [
    (0x80, "attackfast40"),
    (0x100, "zf2-e307"),
    (0x200, "attackup40"),
    (0x40000, "SuperSoldier"),
    (0x800000, "Tornado"),
    (0x4000000, "Dodge"),
    (0x80000, "BodyShield"),
];

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
    requests: Sender<GeometryJob>,
    results: Receiver<GeometryResult>,
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
    Minimap {
        map_id: u32,
        result: std::result::Result<Option<crate::minimap::Minimap>, String>,
    },
}

struct TextureLoader {
    requests: Sender<String>,
    results: Receiver<LoadedTexture>,
}

struct UiTextureLoader {
    requests: Sender<UiTextureJob>,
    results: Receiver<LoadedTexture>,
}

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
        action_context_hover: Option<usize>,
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
        selected_skill: Option<u32>,
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
        player_shop_items: Vec<crate::network::PlayerShopItem>,
        own_character_id: Option<u32>,
        player_shop_open: bool,
        player_shop_price: String,
        player_shop_price_focused: bool,
        hud_config: crate::hud::HudConfig,
        bounty: Option<crate::bounty_board::BountyView>,
        store: Option<crate::store_panel::StoreView>,
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
                monster,
                bounty,
                health,
                maximum_health,
                name_colour,
                ..
            } => {
                if monster {
                    crate::labels::monster_image(&name, health, maximum_health, bounty)
                } else {
                    crate::labels::actor_image(&name, name_colour)
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
                action_context_hover,
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
                action_context_hover,
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
                ..
            } => crate::shop::status_base_image(assets, active_tab, weapon_filter, close_hovered),
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
                selected_skill,
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
                selected_skill,
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
                player_shop_items,
                own_character_id,
                player_shop_open,
                player_shop_price,
                player_shop_price_focused,
                hud_config,
                bounty,
                store,
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
                &player_shop_items,
                own_character_id,
                player_shop_open,
                &player_shop_price,
                player_shop_price_focused,
                hud_config,
                bounty.as_ref(),
                store.as_ref(),
            ),
        };
        image.map_err(|error| error.to_string())
    }
}

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    /// What the surface can present with; VSync off picks from these.
    present_modes: Vec<wgpu::PresentMode>,
    layout: wgpu::BindGroupLayout,
    sprites: wgpu::RenderPipeline,
    actor: wgpu::RenderPipeline,
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
    /// Where the bounty line under the minimap was drawn, for clicks.
    bounty_tracker_rect: Option<[f32; 4]>,
    /// Where the Store icon was drawn, for clicks.
    store_icon_rect: Option<[f32; 4]>,
    /// Where the revive buttons were drawn (and whether each revives
    /// here), for clicks.
    revive_rects: Vec<([f32; 4], bool)>,
    level_up_layers: Option<Vec<LevelUpLayer>>,
    /// Original 3D effects by name; `None` when the effect is unavailable.
    effects: HashMap<String, Option<std::rc::Rc<LoadedEffect>>>,
    last_hud_key: Option<String>,
}
impl Renderer {
    pub async fn new(window: Arc<Window>) -> Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance.create_surface(window.clone())?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await?;
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or("No surface format")?;
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
        let pipeline = |actor| {
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
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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
        let sprites = pipeline(false);
        let actor = pipeline(true);
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
            bounty_tracker_rect: None,
            store_icon_rect: None,
            revive_rects: Vec::new(),
            level_up_layers: None,
            effects: HashMap::new(),
            last_hud_key: None,
        })
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
        if !self.effects.contains_key(name) {
            let loaded = crate::magic::effect(assets, name).and_then(|definition| {
                let layers = definition
                    .layers
                    .iter()
                    .filter_map(|layer| {
                        Some(LevelUpLayer {
                            document: C3Document::parse(&assets.read(&layer.mesh).ok()?).ok()?,
                            texture: layer.texture.clone(),
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
        for layer in &effect.layers {
            self.texture(assets, &layer.texture);
        }
        let (sin, cos) = (-std::f32::consts::FRAC_PI_4).sin_cos();
        let scale = 0.75;
        for layer in &effect.layers {
            if !self.textures.contains_key(&layer.texture) {
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
                                [origin[0] + p.x * scale, origin[1] + screen_y],
                                (0.5 + depth * 0.0005).clamp(0.001, 0.999),
                                viewport,
                            ),
                            uv: [vertex.texture_coordinate.x, vertex.texture_coordinate.y],
                            alpha,
                        }
                    })
                    .collect();
                draws.push(Draw {
                    texture: layer.texture.clone(),
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

        // Query only spatial buckets around the camera. The exact culling
        // test still runs below when quads are built, so the bucket padding
        // cannot change what is rendered.
        let max_margin = STATIC_OBJECT_MARGIN;
        let min_cell = static_grid_cell([camera[0] - max_margin, camera[1] - max_margin]);
        let max_cell = static_grid_cell([
            camera[0] + viewport[0] + max_margin,
            camera[1] + viewport[1] + max_margin,
        ]);
        let mut candidates = Vec::new();
        for y in min_cell.1..=max_cell.1 {
            for x in min_cell.0..=max_cell.0 {
                if let Some(indices) = static_map.grid.get(&(x, y)) {
                    candidates.extend(indices.iter().copied());
                }
            }
        }
        candidates.sort_unstable();
        let indices = candidates
            .into_iter()
            .filter(|index| {
                let sprite = &static_map.static_sprites[*index];
                let p = [sprite.origin[0] - camera[0], sprite.origin[1] - camera[1]];
                let margin = if sprite.depth == f32::NEG_INFINITY {
                    STATIC_GROUND_MARGIN
                } else {
                    STATIC_OBJECT_MARGIN
                };
                p[0] <= viewport[0] + margin
                    && p[1] <= viewport[1] + margin
                    && p[0] >= -margin
                    && p[1] >= -margin
            })
            .collect::<Vec<_>>();
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
        let (request_tx, request_rx) = mpsc::channel::<String>();
        let (result_tx, result_rx) = mpsc::channel::<LoadedTexture>();
        let root = assets.root_path().to_owned();
        thread::Builder::new()
            .name("gravak-asset-loader".to_owned())
            .spawn(move || {
                let mut assets = match Assets::open(&root) {
                    Ok(assets) => assets,
                    Err(error) => {
                        let message = error.to_string();
                        while let Ok(path) = request_rx.recv() {
                            let _ = result_tx.send(LoadedTexture {
                                path,
                                data: LoadedData::Image(Err(message.clone())),
                            });
                        }
                        return;
                    }
                };
                while let Ok(path) = request_rx.recv() {
                    let image = assets.image(&path).map_err(|error| error.to_string());
                    if result_tx
                        .send(LoadedTexture {
                            path,
                            data: LoadedData::Image(image),
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .expect("failed to start asset loader thread");
        self.texture_loader = Some(TextureLoader {
            requests: request_tx,
            results: result_rx,
        });
    }

    fn ensure_ui_texture_loader(&mut self, assets: &Assets) {
        if self.ui_texture_loader.is_some() {
            return;
        }
        let (request_tx, request_rx) = mpsc::channel::<UiTextureJob>();
        let (result_tx, result_rx) = mpsc::channel::<LoadedTexture>();
        let root = assets.root_path().to_owned();
        thread::Builder::new()
            .name("gravak-ui-texture-loader".to_owned())
            .spawn(move || {
                let mut assets = match Assets::open(&root) {
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
                while let Ok(job) = request_rx.recv() {
                    let path = job.path().to_owned();
                    let data = match job {
                        UiTextureJob::Minimap { map_id, .. } => LoadedData::Minimap {
                            map_id,
                            result: crate::minimap::Minimap::load(&mut assets, map_id)
                                .map_err(|error| error.to_string()),
                        },
                        job => LoadedData::Image(job.render(&mut assets)),
                    };
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

    fn ensure_geometry_loader(&mut self) {
        if self.geometry_loader.is_some() {
            return;
        }
        let (request_tx, request_rx) = mpsc::channel::<GeometryJob>();
        let (result_tx, result_rx) = mpsc::channel::<GeometryResult>();
        thread::Builder::new()
            .name("gravak-geometry-loader".to_owned())
            .spawn(move || {
                while let Ok(job) = request_rx.recv() {
                    let (key, vertices) = match job {
                        GeometryJob::Npc { key, model, facing } => {
                            (key, model.vertices_static(facing))
                        }
                        GeometryJob::Character {
                            key,
                            character,
                            part_index,
                            facing,
                        } => (
                            key,
                            character
                                .vertices_idle(part_index, facing)
                                .unwrap_or_default(),
                        ),
                    };
                    if result_tx
                        .send(GeometryResult {
                            key,
                            vertices: Arc::new(vertices),
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .expect("failed to start geometry loader thread");
        self.geometry_loader = Some(GeometryLoader {
            requests: request_tx,
            results: result_rx,
        });
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
        if let Some(loader) = &self.ui_texture_loader {
            while let Ok(loaded) = loader.results.try_recv() {
                self.ready_textures.push_back(loaded);
            }
        }
        while self.texture_loads_this_frame < self.max_texture_loads_per_frame {
            let Some(loaded) = self.ready_textures.pop_front() else {
                break;
            };
            self.loading.remove(&loaded.path);
            self.texture_loads_this_frame += 1;
            match loaded.data {
                LoadedData::Image(Ok(image)) => self.upload(&loaded.path, image),
                LoadedData::Image(Err(error)) => {
                    eprintln!("Skipped texture {}: {error}", loaded.path);
                    self.missing.insert(loaded.path);
                }
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
            || self.texture_loads_this_frame >= self.max_texture_loads_per_frame
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
            self.texture_loads_this_frame += 1;
        }
    }
    fn upload(&mut self, path: &str, image: image::RgbaImage) {
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
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
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
    pub fn camera(&self, map: &Map, player: &Player) -> [f32; 2] {
        let p = map.project(player.position);
        [
            p[0] - self.config.width as f32 * 0.5,
            p[1] - self.config.height as f32 * 0.5,
        ]
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
        self.minimap
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
        combat_numbers: &[(u32, u32, f32)],
        skill_effects: &[SkillEffectInstance],
        ground_loot: &[(u32, [f32; 2], u32, f32)],
        hit_until: f32,
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
        player_shop_items: &[crate::network::PlayerShopItem],
        player_shop_open: bool,
        player_shop_price: &str,
        player_shop_price_focused: bool,
        bounty: Option<&crate::bounty_board::BountyView>,
        store: Option<&crate::store_panel::StoreView>,
        item_context: Option<crate::shop::ItemContext>,
        choice_menu: Option<crate::shop::ChoiceMenu>,
        notice: Option<&str>,
        prompt: Option<crate::shop::PromptDialog>,
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
            }
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
        let viewport = [self.config.width as f32, self.config.height as f32];
        self.ensure_static_map_cache(map);
        let mut draws = Vec::with_capacity(map.sprites.len() / 3 + remote_actors.len() * 2 + 64);

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
            draws.push(Draw {
                texture: path.clone(),
                vertices: quad(p, size, viewport),
                depth_test: false,
                sort_depth: sprite.depth,
            });
        }
        let visible_static_indices = self.visible_static_sprite_indices(map, camera, viewport);
        for index in visible_static_indices.iter().copied() {
            let (texture_path, origin, depth) = {
                let sprite = &self
                    .static_map_cache
                    .as_ref()
                    .expect("static map cache exists")
                    .static_sprites[index];
                (sprite.texture.clone(), sprite.origin, sprite.depth)
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
            draws.push(Draw {
                texture: texture_path,
                vertices: quad(p, texture.size, viewport),
                depth_test: false,
                sort_depth: depth,
            });
        }
        let ground = [viewport[0] * 0.5, viewport[1] * 0.5];
        let (recoil_x, recoil_y) = if time < hit_until {
            // One short, visible flinch pulse. It is independent of the
            // motion catalog so a missing emote cannot become a wave.
            let progress = ((hit_until - time) / 0.45).clamp(0.0, 1.0);
            let pulse = (progress * std::f32::consts::PI).sin();
            (-pulse * 9.0, -pulse * 4.0)
        } else {
            (0.0, 0.0)
        };
        let origin = [ground[0] + recoil_x, ground[1] - player.height + recoil_y];
        let actor_depth = player.position[0] + player.position[1];
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
        if time < hit_until {
            if !self.textures.contains_key("@hit-flash") {
                self.upload("@hit-flash", crate::labels::hit_flash_image()?);
            }
            if let Some(flash) = self.textures.get("@hit-flash") {
                let progress = ((hit_until - time) / 0.45).clamp(0.0, 1.0);
                let size = 46.0 + (1.0 - progress) * 18.0;
                draws.push(Draw {
                    texture: "@hit-flash".into(),
                    vertices: quad(
                        [origin[0] - size * 0.5, origin[1] - size * 0.7],
                        [size, size],
                        viewport,
                    ),
                    depth_test: false,
                    sort_depth: actor_depth + 0.01,
                });
                let _ = flash;
            }
        }
        let static_world_phase_finished = Instant::now();
        let mut remote_pose_cache =
            HashMap::<(u64, usize, i8, usize, u32), Arc<Vec<([f32; 3], [f32; 2])>>>::new();
        let mut remote_monster_pose_cache =
            HashMap::<(u32, u8, i8, u32), Arc<Vec<([f32; 3], [f32; 2])>>>::new();
        let mut remote_pose_build_budget = 1usize;
        // Players have few parts and are few on screen; give them their own
        // budget so walking monsters cannot freeze them in the idle pose.
        let mut remote_player_pose_budget = 4usize;
        for actor in remote_actors.values() {
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
            let held = matches!(actor.action, REMOTE_SIT | REMOTE_DEAD);
            let action = if time < actor.animation_until || held {
                remote_action(actor.action)
            } else {
                Action::Idle
            };
            let action_progress = if held && time >= actor.animation_until {
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
            // A remote jump (action 130) follows the local player's arc.
            if actor.action == 130
                && actor.movement_until > actor.movement_started
                && time < actor.movement_until
            {
                let progress = (time - actor.movement_started)
                    / (actor.movement_until - actor.movement_started);
                let peak = crate::motion_math::jump_motion(
                    actor.movement_from,
                    actor.movement_to,
                    std::time::Duration::ZERO,
                )
                .peak_screen_pixels;
                origin[1] -= crate::motion_math::jump_screen_height(peak, progress);
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
                let dynamic_kind = if actor.action == 160 && time < actor.animation_until {
                    Some(0)
                } else if time < actor.animation_until {
                    Some(if matches!(actor.action, 110 | 111 | 120 | 121) {
                        1
                    } else {
                        2
                    })
                } else {
                    None
                };
                let vertices: Arc<Vec<([f32; 3], [f32; 2])>> = if let Some(kind) = dynamic_kind {
                    let (model_id, _) = monster.static_vertices_key(facing);
                    let facing_sector = (facing / std::f32::consts::FRAC_PI_4).round() as i8;
                    let frame_bucket = (time.max(0.0) * 30.0).floor() as u32;
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
                                        0 => monster.vertices_death(time, facing),
                                        1 => monster.vertices_walk(time, facing),
                                        _ => monster.vertices_attack(time, facing),
                                    })
                                })
                        } else {
                            remote_pose_build_budget -= 1;
                            let vertices = Arc::new(match kind {
                                0 => monster.vertices_death(time, facing),
                                1 => monster.vertices_walk(time, facing),
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
                            let vertices = Arc::new(remote_character.vertices(
                                part,
                                time,
                                action,
                                facing,
                                action_progress,
                            ));
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
            }
            // A hovered player shows their face, name and health instead
            // of the plain name.
            if hovered_player == Some(actor.id) && !actor.npc && !monster && !actor.name.is_empty() {
                let name_colour = crate::labels::name_colour(actor.status_flags);
                let key = format!(
                    "@player-plate-{}-{}-{}-{}-{:?}",
                    actor.id, actor.mesh, actor.health, actor.maximum_health, name_colour
                );
                if !self.textures.contains_key(&key) {
                    let face = player_face(assets, actor.mesh, crate::labels::PLAYER_FACE_SIZE);
                    self.upload(
                        &key,
                        crate::labels::player_plate(
                            face.as_ref(),
                            &actor.name,
                            name_colour,
                            actor.health,
                            actor.maximum_health,
                        )?,
                    );
                }
                if let Some(plate) = self.textures.get(&key) {
                    draws.push(Draw {
                        texture: key,
                        vertices: quad(
                            [
                                (origin[0] - plate.size[0] * 0.5).round(),
                                (origin[1] - 160.0).round(),
                            ],
                            plate.size,
                            viewport,
                        ),
                        depth_test: false,
                        sort_depth: WORLD_OVERLAY_SORT,
                    });
                }
            } else if !actor.name.is_empty() && (hud_config.show_names || monster) {
                let name_colour = crate::labels::name_colour(actor.status_flags);
                // The Guild War pole and gates show their life like monsters.
                let monster = monster || (actor.npc && matches!(actor.npc_kind, 10 | 26));
                let bounty_target = monster
                    && bounty
                        .and_then(|view| view.board.active())
                        .is_some_and(|task| task.monster_name == actor.name);
                let label_key = if monster {
                    format!("@monster-label-{}-{}-{bounty_target}", actor.id, actor.health)
                } else {
                    format!("@actor-label-{}-{:?}", actor.id, name_colour)
                };
                if !self.textures.contains_key(&label_key) && !self.loading.contains(&label_key) {
                    self.queue_ui_texture(
                        assets,
                        UiTextureJob::ActorLabel {
                            path: label_key.clone(),
                            name: actor.name.clone(),
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
                                        80.0
                                    } else if monster {
                                        68.0
                                    } else {
                                        152.0
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
                if !actor.npc && !monster && actor.status_flags & VIP_FLAG != 0 {
                    self.push_vip_tag([origin[0], origin[1] - 152.0], viewport, &mut draws)?;
                }
            }
        }
        let remote_world_phase_finished = Instant::now();
        for (index, (target_id, value, expires)) in combat_numbers.iter().enumerate() {
            if *expires <= time {
                continue;
            }
            let position = remote_actors
                .get(target_id)
                .map(|target| target.position)
                .unwrap_or(player.position);
            let projected = map.project(position);
            let origin = [projected[0] - camera[0], projected[1] - camera[1]];
            if !world_point_visible(origin, viewport, 192.0) {
                continue;
            }
            let key = format!("@damage-{value}");
            if !self.textures.contains_key(&key) {
                self.upload(&key, crate::labels::damage_image(*value)?);
            }
            if let Some(texture) = self.textures.get(&key) {
                let rise = (1.0 - (*expires - time).clamp(0.0, 1.0)) * 28.0;
                draws.push(Draw {
                    texture: key,
                    vertices: quad(
                        [
                            origin[0] - texture.size[0] * 0.5,
                            origin[1] - 115.0 - rise - index as f32 * 4.0,
                        ],
                        texture.size,
                        viewport,
                    ),
                    depth_test: false,
                    sort_depth: WORLD_OVERLAY_SORT,
                });
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
            for (name, actor) in [
                (magic.intone_effect.as_deref(), effect.attacker_id),
                (magic.target_effect.as_deref(), effect.target_id),
            ] {
                let Some(name) = name else {
                    continue;
                };
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
                    elapsed,
                    position[0] + position[1] + 0.05,
                    viewport,
                    &mut draws,
                );
            }
        }
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
        let mut status_effects = vec![(player.position, status_flags)];
        status_effects.extend(remote_actors.values().map(|actor| (actor.position, actor.status_flags)));
        for (position, flags) in status_effects {
            if flags == 0 {
                continue;
            }
            let projected = map.project(position);
            let origin = [projected[0] - camera[0], projected[1] - camera[1]];
            if !world_point_visible(origin, viewport, 256.0) {
                continue;
            }
            for (bit, name) in STATUS_EFFECTS {
                if flags & bit != 0 {
                    self.push_looping_effect(assets, name, origin, time, position[0] + position[1] + 0.04, viewport, &mut draws);
                }
            }
        }
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
                self.upload(&name_key, crate::labels::loot_name_image(&name)?);
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
        let name_colour = crate::labels::name_colour(status_flags);
        let label_key = format!(
            "@label-{}-{}-{}-{:?}",
            character.name, character.health, character.maximum_health, name_colour
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
            self.push_vip_tag([origin[0], origin[1] - 152.0], viewport, &mut draws)?;
        }
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
        if minimap_visible {
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
        let hud_hover = crate::hud::hovered_button(viewport, cursor);
        let action_context_hover = action_context_slot
            .and_then(|slot| crate::hud::action_context_option_at(viewport, cursor, slot));
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
        action_context_hover.hash(&mut hud_hasher);
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
                    action_context_hover,
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
        // Dead: the revive countdown above the KO counter, then the buttons
        // there: Revive, and Revive here under Heaven's Blessing.
        self.revive_rects.clear();
        let mut revive_keys: Vec<(String, Option<bool>)> = Vec::new();
        match revive {
            Some((remaining, _)) if remaining > 0 => revive_keys.push((format!("@revive-{remaining}"), None)),
            Some((_, here)) => {
                revive_keys.push(("@revive-button".to_owned(), Some(false)));
                if here {
                    revive_keys.push(("@revive-here-button".to_owned(), Some(true)));
                }
            },
            None => {},
        }
        let [right, mut bottom] = crate::hud::revive_anchor(viewport);
        for (key, button) in &revive_keys {
            if !self.textures.contains_key(key) {
                let image = match (button, revive) {
                    (None, Some((remaining, _))) => crate::hud::revive_countdown_image(remaining)?,
                    (Some(true), _) => crate::hud::hud_style_button(assets, "Revive here")?,
                    _ => crate::hud::hud_style_button(assets, "Revive")?,
                };
                self.upload(key, image);
            }
            if let Some(texture) = self.textures.get(key) {
                let position = [right - texture.size[0], bottom - texture.size[1]];
                if let Some(here) = button {
                    self.revive_rects.push(([position[0], position[1], texture.size[0], texture.size[1]], *here));
                }
                draws.push(Draw {
                    texture: key.clone(),
                    vertices: quad(position, texture.size, viewport),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
                bottom = position[1] - 6.0;
            }
        }
        let countdown_key = revive_keys
            .iter()
            .find(|(_, button)| button.is_none())
            .map(|(key, _)| key.as_str());
        self.prune_textures("@revive-", &[countdown_key, Some("@revive-button"), Some("@revive-here-button")]);
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
            let top = self.minimap.as_ref().map_or(180.0, |minimap| minimap.bottom(viewport)) + 8.0;
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
        // The Store button, by the round buttons over the orb.
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
        // Each status message is its own texture; keep only the latest.
        let current_feedback = self
            .feedback_signature
            .map(|signature| format!("@network-feedback-{signature}"));
        self.prune_textures("@network-feedback-", &[current_feedback.as_deref()]);
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
            let base_key = format!(
                "@original-character-base-{status_tab:?}-{weapon_filter:?}-{close_hovered}"
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
                        selected_skill: skill_hud.active_skill,
                    },
                );
                self.status_signature = Some(signature);
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
                        "@status-equipment-tooltip-{}-{}-{}-{}-{}-{}-{}-{}-{}",
                        item.position,
                        item.item_type,
                        item.durability,
                        item.max_durability,
                        item.plus,
                        item.gem_one,
                        item.gem_two,
                        item.rarity,
                        item.rarity_roll
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
                                item.rarity_roll,
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
            let key = format!("@npc-dialog-{}", hasher.finish());
            if !self.textures.contains_key(&key) {
                self.upload(
                    &key,
                    crate::npc_dialog::image(assets, messages, options, avatar, hovered)?,
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
            player_shop_items.hash(&mut hasher);
            player_shop_open.hash(&mut hasher);
            player_shop_price.hash(&mut hasher);
            player_shop_price_focused.hash(&mut hasher);
            hud_config.hash(&mut hasher);
            if panel == crate::shop::QuickPanel::Quest {
                bounty.hash(&mut hasher);
            }
            if matches!(panel, crate::shop::QuickPanel::Store | crate::shop::QuickPanel::Vip) {
                store.hash(&mut hasher);
                // VIP time left reads in minutes.
                (std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
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
                .map(|actor| actor.name.as_str());
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
                        player_shop_items: player_shop_items.to_vec(),
                        own_character_id: network_character_id,
                        player_shop_open,
                        player_shop_price: player_shop_price.to_owned(),
                        player_shop_price_focused,
                        hud_config,
                        bounty: bounty.cloned(),
                        store: store.cloned(),
                    },
                );
                self.quick_panel_signature = Some(signature);
            }
            if let Some(texture) = self.textures.get(&key) {
                draws.push(Draw {
                    texture: key,
                    vertices: quad(
                        crate::shop::quick_panel_position(viewport, panel),
                        texture.size,
                        viewport,
                    ),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
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
            shop.page.hash(&mut hasher);
            shop.selected.hash(&mut hasher);
            shop.repair_hover.hash(&mut hasher);
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
                    let tooltip_key = format!("@shop-tooltip-{item_id}");
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
                "@inventory-tooltip-{}-{}-{}-{}-{}-{}-{}",
                item.uid,
                item.plus,
                item.gem_one,
                item.gem_two,
                item.item_type,
                item.rarity,
                item.rarity_roll
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
                        item.rarity_roll,
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
                    "@action-item-tooltip-{}-{}-{}-{}-{}-{}-{}",
                    panel.item.uid,
                    panel.item.plus,
                    panel.item.gem_one,
                    panel.item.gem_two,
                    panel.item.item_type,
                    panel.item.rarity,
                    panel.item.rarity_roll
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
                            panel.item.rarity_roll,
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
            for (draw, (start, end)) in draws.iter().zip(&ranges) {
                let Some(texture) = self.textures.get(&draw.texture) else {
                    continue;
                };
                if draw.depth_test && draw.sort_depth == f32::INFINITY {
                    let [x, y, width, height] = crate::shop::status_character_clip([
                        self.config.width as f32,
                        self.config.height as f32,
                    ]);
                    pass.set_scissor_rect(x, y, width, height);
                } else {
                    pass.set_scissor_rect(0, 0, self.config.width, self.config.height);
                }
                pass.set_pipeline(if draw.depth_test {
                    &self.actor
                } else {
                    &self.sprites
                });
                pass.set_bind_group(0, &texture.bind, &[]);
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

fn coalesce_adjacent_draws(draws: Vec<Draw>) -> Vec<Draw> {
    let mut merged: Vec<Draw> = Vec::with_capacity(draws.len());
    for draw in draws {
        if let Some(previous) = merged.last_mut()
            && previous.texture == draw.texture
            && previous.depth_test == draw.depth_test
            && previous.sort_depth == draw.sort_depth
        {
            previous.vertices.extend(draw.vertices);
        } else {
            merged.push(draw);
        }
    }
    merged
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
const TRANSIENT_PREFIXES: [&str; 14] = [
    "@actor-label-",
    "@monster-label-",
    "@label-",
    "@damage-",
    "@loot-name-",
    "@minimap-pin-name-",
    "@choice-menu-",
    "@prompt-",
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

/// Remote action codes whose last frame is held: sitting (250) and a
/// dead player (161, a client-side code for the 401 fall).
pub(crate) const REMOTE_SIT: u8 = 250;
pub(crate) const REMOTE_DEAD: u8 = 161;

fn remote_action(value: u8) -> Action {
    match value {
        REMOTE_SIT => Action::Sit,
        REMOTE_DEAD => Action::Die,
        110 => Action::WalkLeft,
        111 => Action::WalkRight,
        120 => Action::RunLeft,
        121 => Action::RunRight,
        130 => Action::Jump,
        140 => Action::Attack,
        150 => Action::Hit,
        220 => Action::Pickup,
        _ => Action::Idle,
    }
}

fn should_hide_dead_monster(
    health: u16,
    action: u8,
    animation_until: f32,
    respawn_until: f32,
    time: f32,
) -> bool {
    health == 0 && action == 160 && time >= animation_until && time >= respawn_until
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

#[cfg(test)]
mod tests {
    use super::{
        Draw, coalesce_adjacent_draws, quad, should_hide_dead_monster, world_point_visible,
    };

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
        assert!(!should_hide_dead_monster(0, 160, 1.2, 0.0, 1.19));
        assert!(should_hide_dead_monster(0, 160, 1.2, 0.0, 1.2));
        assert!(!should_hide_dead_monster(100, 160, 1.2, 0.0, 2.0));
        assert!(!should_hide_dead_monster(0, 160, 1.2, 3.0, 2.0));
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
