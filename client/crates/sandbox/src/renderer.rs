// Source: @digitalm1nd on x.com / _dek on Discord — Preservation Conquer project — https://discord.gg/CvKPXEHYRY
use crate::{
    action::Action,
    assets::{Assets, Result},
    character::Character,
    map::Map,
    movement::Player,
    network::RemoteActor,
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use wgpu::util::DeviceExt;
use winit::window::Window;
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    uv: [f32; 2],
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
pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    layout: wgpu::BindGroupLayout,
    sprites: wgpu::RenderPipeline,
    actor: wgpu::RenderPipeline,
    depth: wgpu::TextureView,
    textures: HashMap<String, Texture>,
    missing: HashSet<String>,
    minimap: Option<crate::minimap::Minimap>,
    minimap_map: Option<u32>,
    shop_signature: Option<u64>,
    inventory_signature: Option<u64>,
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
        config.present_mode = wgpu::PresentMode::AutoVsync;
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
                        array_stride: 20,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x2],
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
        Ok(Self {
            surface,
            device,
            queue,
            config,
            layout,
            sprites,
            actor,
            depth,
            textures: HashMap::new(),
            missing: HashSet::new(),
            minimap: None,
            minimap_map: None,
            shop_signature: None,
            inventory_signature: None,
        })
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
    fn texture(&mut self, assets: &mut Assets, path: &str) {
        if self.textures.contains_key(path) || self.missing.contains(path) {
            return;
        }
        let image = match assets.image(path) {
            Ok(image) => image,
            Err(error) => {
                eprintln!("Skipped texture {path}: {error}");
                self.missing.insert(path.to_owned());
                return;
            }
        };
        self.upload(path, image);
    }
    fn upload(&mut self, path: &str, image: image::RgbaImage) {
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
        let filter = if matches!(path, "@label" | "@build") {
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
                size: [image.width() as f32, image.height() as f32],
            },
        );
    }
    pub fn camera(&self, map: &Map, player: &Player) -> [f32; 2] {
        let p = map.project(player.position);
        [
            p[0] - self.config.width as f32 * 0.5,
            p[1] - self.config.height as f32 * 0.5,
        ]
    }
    pub fn minimap_pointer(&mut self, position: [f32; 2], down: bool) -> bool {
        self.minimap.as_mut().is_some_and(|minimap| {
            minimap.pointer(
                position,
                down,
                [self.config.width as f32, self.config.height as f32],
            )
        })
    }
    pub fn draw(
        &mut self,
        assets: &mut Assets,
        map: &Map,
        character: &Character,
        player: &Player,
        remote_actors: &HashMap<u32, RemoteActor>,
        remote_models: &HashMap<u32, Character>,
        remote_npcs: &HashMap<u32, crate::character::NpcModel>,
        dialog: Option<(&[String], &[(u8, String)])>,
        shop: Option<&crate::shop::ShopState>,
        inventory: &[crate::network::OwnedItem],
        silver: u32,
        dragging_shop_item: Option<u32>,
        dragging_inventory_item: Option<crate::network::OwnedItem>,
        cursor: [f32; 2],
        time: f32,
    ) -> Result<()> {
        let camera = self.camera(map, player);
        let viewport = [self.config.width as f32, self.config.height as f32];
        let mut draws = Vec::new();
        for sprite in &map.sprites {
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
        let ground = [viewport[0] * 0.5, viewport[1] * 0.5];
        let origin = [ground[0], ground[1] - player.height];
        let actor_depth = player.position[0] + player.position[1];
        const SHADOW: &str = "data/pic/Shadow/simpleShadow.dds";
        if !self.textures.contains_key("@shadow") {
            let shadow = assets.image(SHADOW)?;
            let mut min = [shadow.width(), shadow.height()];
            let mut max = [0, 0];
            for (x, y, p) in shadow.enumerate_pixels() {
                if p[3] != 0 {
                    min[0] = min[0].min(x);
                    min[1] = min[1].min(y);
                    max[0] = max[0].max(x);
                    max[1] = max[1].max(y);
                }
            }
            if max[0] >= min[0] && max[1] >= min[1] {
                self.upload(
                    "@shadow",
                    image::imageops::crop_imm(
                        &shadow,
                        min[0],
                        min[1],
                        max[0] - min[0] + 1,
                        max[1] - min[1] + 1,
                    )
                    .to_image(),
                );
            }
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
        for part in &character.parts {
            self.texture(assets, &part.texture);
            let vertices =
                character.vertices(part, time, player.action, player.facing, player.progress());
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
        for actor in remote_actors.values() {
            let action = if time < actor.animation_until {
                remote_action(actor.action)
            } else {
                Action::Idle
            };
            let projected = map.project(actor.position);
            let origin = [projected[0] - camera[0], projected[1] - camera[1]];
            let depth = actor.position[0] + actor.position[1];
            let facing = -f32::from((actor.direction % 8) + 1) * std::f32::consts::FRAC_PI_4;
            if let Some(npc) = remote_npcs.get(&actor.id) {
                self.texture(assets, &npc.texture);
                let vertices = npc.vertices(time, facing);
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
                        }
                    })
                    .collect();
                draws.push(Draw {
                    texture: npc.texture.clone(),
                    vertices: triangles,
                    depth_test: true,
                    sort_depth: depth,
                });
            } else if let Some(remote_character) = remote_models.get(&actor.id) {
                for part in &remote_character.parts {
                    self.texture(assets, &part.texture);
                    let vertices = remote_character.vertices(part, time, action, facing, None);
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
            if !actor.name.is_empty() {
                let label_key = format!("@actor-label-{}", actor.id);
                if !self.textures.contains_key(&label_key) {
                    self.upload(&label_key, crate::labels::actor_image(&actor.name)?);
                }
                if let Some(label) = self.textures.get(&label_key) {
                    draws.push(Draw {
                        texture: label_key,
                        vertices: quad(
                            [
                                (origin[0] - label.size[0] * 0.5).round(),
                                (origin[1] - if actor.npc { 80.0 } else { 152.0 }).round(),
                            ],
                            label.size,
                            viewport,
                        ),
                        depth_test: false,
                        sort_depth: f32::INFINITY,
                    });
                }
            }
        }
        if !self.textures.contains_key("@label") {
            self.upload("@label", crate::labels::image(character)?);
        }
        draws.push(Draw {
            texture: "@label".into(),
            vertices: quad(
                [(origin[0] - 128.0).round(), (origin[1] - 152.0).round()],
                [256.0, 40.0],
                viewport,
            ),
            depth_test: false,
            sort_depth: f32::INFINITY,
        });
        if !self.textures.contains_key("@build") {
            self.upload("@build", crate::labels::build_banner()?);
        }
        draws.push(Draw {
            texture: "@build".into(),
            vertices: quad([4.0, 3.0], self.textures["@build"].size, viewport),
            depth_test: false,
            sort_depth: f32::INFINITY,
        });
        if self.minimap_map != Some(map.id) {
            self.minimap_map = Some(map.id);
            self.minimap = None;
            match crate::minimap::Minimap::load(assets, map.id) {
                Ok(Some(minimap)) => {
                    for (key, image) in &minimap.images {
                        self.upload(key, image.clone());
                    }
                    self.minimap = Some(minimap);
                }
                Ok(None) => {}
                Err(error) => eprintln!("Minimap unavailable: {error}"),
            }
        }
        if let Some(minimap) = &self.minimap {
            for item in minimap.quads(map, player.position, camera, viewport) {
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
        if let Some((messages, options)) = dialog {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            messages.hash(&mut hasher);
            options.hash(&mut hasher);
            let key = format!("@dialog-{}", hasher.finish());
            if !self.textures.contains_key(&key) {
                self.upload(&key, crate::labels::dialog_image(messages, options)?);
            }
            if let Some(texture) = self.textures.get(&key) {
                let size = texture.size;
                draws.push(Draw {
                    texture: key,
                    vertices: quad(
                        [
                            ((viewport[0] - size[0]) * 0.5).max(8.0),
                            (viewport[1] - size[1] - 24.0).max(8.0),
                        ],
                        size,
                        viewport,
                    ),
                    depth_test: false,
                    sort_depth: f32::INFINITY,
                });
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
            let signature = hasher.finish();
            let key = "@shop-panel";
            if self.shop_signature != Some(signature) {
                self.upload(key, crate::shop::panel_image(assets, shop)?);
                self.shop_signature = Some(signature);
            }
            if let Some(texture) = self.textures.get(key) {
                draws.push(Draw {
                    texture: key.into(),
                    vertices: quad(crate::shop::panel_position(viewport), texture.size, viewport),
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
            let inventory_signature = inventory_hasher.finish();
            if self.inventory_signature != Some(inventory_signature) {
                self.upload(
                    "@inventory-panel",
                    crate::shop::inventory_image(assets, inventory, silver)?,
                );
                self.inventory_signature = Some(inventory_signature);
            }
            if let Some(texture) = self.textures.get("@inventory-panel") {
                draws.push(Draw {
                    texture: "@inventory-panel".into(),
                    vertices: quad(crate::shop::inventory_position(viewport), texture.size, viewport),
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
        if let (Some(item_id), Some(shop)) = (dragging_shop_item, shop) {
            let key = format!("@shop-drag-item-{item_id}");
            if !self.textures.contains_key(&key)
                && let Some(icon) = shop.icon_image(item_id).cloned()
            {
                self.upload(&key, icon);
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
        if let Some(item) = dragging_inventory_item {
            let key = format!("@inventory-drag-item-{}", item.uid);
            if !self.textures.contains_key(&key) {
                self.upload(&key, crate::shop::item_icon_image(assets, item.item_type)?);
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
        draws.sort_by(|a, b| a.sort_depth.total_cmp(&b.sort_depth));
        let buffers: Vec<_> = draws
            .iter()
            .map(|draw| {
                self.device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: None,
                        contents: bytemuck::cast_slice(&draw.vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    })
            })
            .collect();
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
            for (draw, buffer) in draws.iter().zip(&buffers) {
                let Some(texture) = self.textures.get(&draw.texture) else {
                    continue;
                };
                pass.set_pipeline(if draw.depth_test {
                    &self.actor
                } else {
                    &self.sprites
                });
                pass.set_bind_group(0, &texture.bind, &[]);
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..draw.vertices.len() as u32, 0..1);
            }
        }
        self.queue.submit([encoder.finish()]);
        self.queue.present(frame);
        Ok(())
    }
}

fn remote_action(value: u8) -> Action {
    match value {
        110 => Action::WalkLeft,
        111 => Action::WalkRight,
        120 => Action::RunLeft,
        121 => Action::RunRight,
        130 => Action::Jump,
        _ => Action::Idle,
    }
}
fn clip(p: [f32; 2], z: f32, viewport: [f32; 2]) -> [f32; 3] {
    [
        p[0] / viewport[0] * 2.0 - 1.0,
        1.0 - p[1] / viewport[1] * 2.0,
        z,
    ]
}
fn quad(p: [f32; 2], size: [f32; 2], viewport: [f32; 2]) -> Vec<Vertex> {
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
        })
        .to_vec()
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
    use super::quad;

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
}
