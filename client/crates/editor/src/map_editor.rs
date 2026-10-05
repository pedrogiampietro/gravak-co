//! The map editor: any map of `GameMap.dat` drawn as the client draws it,
//! with its cells, portals and our objects (`content/maps/<id>.toml`), which
//! can be placed, moved and given blocked cells.

use crate::project::{Project, import_object_image};
use eframe::egui::{
    self, Color32, Key, PointerButton, Pos2, Rect, RichText, Sense, Stroke, TextureHandle, Vec2,
};
use gravak_content::maps::{self, MapObject, MapObjects};
use sandbox_data::map::{ComposedMap, GameMapCatalog};
use std::collections::HashMap;

/// Textures decoded per frame at most, so a map opens without a long freeze.
const LOADS_PER_FRAME: usize = 48;
/// Cells drawn at most for the overlay.
const MAX_OVERLAY_CELLS: i64 = 60_000;

const BLOCKED: Color32 = Color32::from_rgba_premultiplied(120, 20, 20, 90);
const OURS_BLOCKED: Color32 = Color32::from_rgba_premultiplied(140, 90, 0, 120);
const OURS_OUTLINE: Color32 = Color32::from_rgb(255, 150, 30);
const SELECTED: Color32 = Color32::from_rgb(255, 210, 60);

struct OpenMap {
    map: ComposedMap,
    objects: MapObjects,
    dirty: bool,
}

#[derive(Default)]
struct Textures {
    /// By asset path (installation) or `content:` path; `None` when it could
    /// not be read.
    loaded: HashMap<String, Option<(TextureHandle, [u32; 2])>>,
    loads_this_frame: usize,
    pending: bool,
}

impl Textures {
    fn get(
        &mut self,
        ctx: &egui::Context,
        key: &str,
        load: impl FnOnce() -> Option<image::RgbaImage>,
    ) -> Option<(TextureHandle, [u32; 2])> {
        if let Some(entry) = self.loaded.get(key) {
            return entry.clone();
        }
        if self.loads_this_frame >= LOADS_PER_FRAME {
            self.pending = true;
            return None;
        }
        self.loads_this_frame += 1;
        let entry = load().map(|image| {
            let size = [image.width(), image.height()];
            let colour = egui::ColorImage::from_rgba_unmultiplied(
                [size[0] as usize, size[1] as usize],
                image.as_raw(),
            );
            (
                ctx.load_texture(key, colour, egui::TextureOptions::LINEAR),
                size,
            )
        });
        self.loaded.insert(key.to_owned(), entry.clone());
        entry
    }
}

pub struct MapEditor {
    catalog: Vec<(u32, String)>,
    filter: String,
    open: Option<OpenMap>,
    camera: Vec2,
    zoom: f32,
    textures: Textures,
    show_scenery: bool,
    show_cells: bool,
    show_portals: bool,
    selected: Option<usize>,
    /// While dragging an object: the cell offset from the cursor to its base.
    grab: Option<[i32; 2]>,
    /// An imported image waiting for a click on the map.
    placing: Option<String>,
    image_path: String,
    clear_background: bool,
    goto: [i32; 2],
    hover: Option<[i32; 2]>,
}

impl MapEditor {
    pub fn new() -> Self {
        Self {
            catalog: Vec::new(),
            filter: String::new(),
            open: None,
            camera: Vec2::ZERO,
            zoom: 0.5,
            textures: Textures::default(),
            show_scenery: true,
            show_cells: false,
            show_portals: true,
            selected: None,
            grab: None,
            placing: None,
            image_path: String::new(),
            clear_background: true,
            goto: [0, 0],
            hover: None,
        }
    }

    /// The installation changed: read its map list again and close the map.
    pub fn refresh(&mut self, project: &mut Project) {
        self.catalog = project
            .installation
            .read("ini/GameMap.dat")
            .ok()
            .and_then(|bytes| GameMapCatalog::parse(&bytes).ok())
            .map(|catalog| {
                let mut maps = catalog
                    .records()
                    .map(|record| (record.id, record.dmap_path.as_str().to_owned()))
                    .collect::<Vec<_>>();
                maps.sort_by_key(|(id, _)| *id);
                maps
            })
            .unwrap_or_default();
        self.open = None;
        self.textures = Textures::default();
        self.selected = None;
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, project: &mut Project, status: &mut String) {
        if self.catalog.is_empty() {
            self.refresh(project);
        }
        self.textures.loads_this_frame = 0;
        self.textures.pending = false;
        egui::Panel::left("map-list")
            .resizable(true)
            .default_size(230.0)
            .show(ui, |ui| {
                self.map_list(ui, project, status);
            });
        egui::Panel::right("map-objects")
            .resizable(true)
            .default_size(300.0)
            .show(ui, |ui| {
                self.objects_panel(ui, project, status);
            });
        egui::CentralPanel::no_frame().show(ui, |ui| {
            if self.open.is_some() {
                self.view(ui, project, status);
            } else {
                ui.centered_and_justified(|ui| {
                    ui.label("Escolha um mapa na lista (Twin City é o 1002).")
                });
            }
        });
        if self.textures.pending {
            ui.ctx().request_repaint();
        }
    }

    fn map_list(&mut self, ui: &mut egui::Ui, project: &mut Project, status: &mut String) {
        ui.horizontal(|ui| {
            ui.label("Buscar");
            ui.text_edit_singleline(&mut self.filter);
        });
        ui.separator();
        let filter = self.filter.trim().to_lowercase();
        let rows = self
            .catalog
            .iter()
            .filter(|(id, path)| {
                filter.is_empty()
                    || id.to_string().contains(&filter)
                    || path.to_lowercase().contains(&filter)
            })
            .cloned()
            .collect::<Vec<_>>();
        let current = self.open.as_ref().map(|open| open.map.id);
        let row_height = ui.spacing().interact_size.y;
        egui::ScrollArea::vertical().auto_shrink(false).show_rows(
            ui,
            row_height,
            rows.len(),
            |ui, range| {
                for (id, path) in &rows[range] {
                    let name = path.rsplit('/').next().unwrap_or(path);
                    let mut text = RichText::new(format!("{id:>5}  {name}")).monospace();
                    if maps::map_file(&project.content, *id).exists() {
                        text = text.color(SELECTED);
                    }
                    if ui.selectable_label(current == Some(*id), text).clicked() {
                        self.open_map(project, *id, status);
                    }
                }
            },
        );
    }

    fn open_map(&mut self, project: &mut Project, id: u32, status: &mut String) {
        if self.open.as_ref().is_some_and(|open| open.dirty) {
            *status = "Salve (ou descarte) as mudanças deste mapa antes de abrir outro.".to_owned();
            return;
        }
        // Only the installation: our objects come from content/, not from a
        // build that may be old.
        let mut read = |name: &str| {
            project
                .installation
                .read(name)
                .map_err(|error| error.to_string())
        };
        let map = match ComposedMap::load(&mut read, id) {
            Ok(map) => map,
            Err(error) => {
                *status = format!("Mapa {id}: {error}");
                return;
            }
        };
        let objects = match maps::load(&project.content, id) {
            Ok(objects) => objects,
            Err(error) => {
                *status = format!("Mapa {id}: {error}");
                return;
            }
        };
        *status = format!(
            "Mapa {id}: {}×{} células, {} objetos nossos",
            map.data.width,
            map.data.height,
            objects.objects.len()
        );
        self.camera = Vec2::new(map.extent[0] * 0.5, map.extent[1] * 0.5);
        self.goto = [map.data.width as i32 / 2, map.data.height as i32 / 2];
        self.open = Some(OpenMap {
            map,
            objects,
            dirty: false,
        });
        self.textures = Textures::default();
        self.selected = None;
        self.placing = None;
    }

    fn center_on(&mut self, cell: [i32; 2]) {
        if let Some(open) = &self.open {
            let p = open.map.project([cell[0] as f32, cell[1] as f32]);
            self.camera = Vec2::new(p[0], p[1]);
        }
    }

    fn objects_panel(&mut self, ui: &mut egui::Ui, project: &mut Project, status: &mut String) {
        let Some(open) = self.open.as_mut() else {
            ui.label("Nenhum mapa aberto.");
            return;
        };
        ui.heading(format!("Mapa {}", open.map.id));
        ui.horizontal(|ui| {
            if ui
                .add_enabled(open.dirty, egui::Button::new("💾 Salvar mapa"))
                .clicked()
            {
                match maps::save(&project.content, open.map.id, &open.objects) {
                    Ok(()) => {
                        open.dirty = false;
                        *status = format!(
                            "Mapa {} salvo em {}. Rode o Build para o jogo usar.",
                            open.map.id,
                            maps::map_file(&project.content, open.map.id).display()
                        );
                    }
                    Err(error) => *status = format!("Erro ao salvar: {error}"),
                }
            }
            if ui
                .add_enabled(open.dirty, egui::Button::new("Descartar"))
                .clicked()
            {
                open.objects = maps::load(&project.content, open.map.id).unwrap_or_default();
                open.dirty = false;
                self.selected = None;
            }
        });
        ui.separator();
        ui.label("Ver");
        ui.checkbox(&mut self.show_scenery, "Cenário (objetos do mapa)");
        ui.checkbox(&mut self.show_cells, "Células bloqueadas");
        ui.checkbox(&mut self.show_portals, "Portais");
        ui.small("Setas ou WASD andam pelo mapa (Shift: mais rápido); + e - dão zoom; botão direito arrasta.");
        ui.horizontal(|ui| {
            ui.label("Ir para");
            ui.add(egui::DragValue::new(&mut self.goto[0]).prefix("x "));
            ui.add(egui::DragValue::new(&mut self.goto[1]).prefix("y "));
            if ui.button("Ir").clicked() {
                let goto = self.goto;
                self.center_on(goto);
            }
        });
        ui.separator();

        ui.label(RichText::new("Novo objeto").strong());
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.image_path)
                    .hint_text("imagem PNG…")
                    .desired_width(170.0),
            );
            if ui.button("Importar").clicked() {
                let path = std::path::PathBuf::from(self.image_path.trim().trim_matches('"'));
                self.import(project, &path, status);
            }
        });
        ui.checkbox(&mut self.clear_background, "Tirar o fundo da imagem");
        ui.small("ou arraste a imagem para o mapa");
        if let Some(image) = &self.placing {
            ui.label(RichText::new(format!("Clique no mapa para colocar {image}")).color(SELECTED));
            if ui.button("Cancelar").clicked() {
                self.placing = None;
            }
        }
        ui.separator();

        let Some(open) = self.open.as_mut() else {
            return;
        };
        ui.label(
            RichText::new(format!("Objetos nossos ({})", open.objects.objects.len())).strong(),
        );
        let mut go_to = None;
        for (index, object) in open.objects.objects.iter().enumerate() {
            let label = if object.name.is_empty() {
                object.image.clone()
            } else {
                object.name.clone()
            };
            let response = ui.selectable_label(
                self.selected == Some(index),
                format!("{label}  ({}, {})", object.x, object.y),
            );
            if response.clicked() {
                self.selected = Some(index);
            }
            if response.double_clicked() {
                go_to = Some([object.x, object.y]);
            }
        }
        if let Some(cell) = go_to {
            self.center_on(cell);
        }
        let Some(open) = self.open.as_mut() else {
            return;
        };
        let Some(index) = self
            .selected
            .filter(|index| *index < open.objects.objects.len())
        else {
            ui.small("Clique num objeto (na lista ou no mapa) para editar.");
            return;
        };
        ui.separator();
        let object = &mut open.objects.objects[index];
        let before = object.clone();
        egui::Grid::new("object").num_columns(2).show(ui, |ui| {
            ui.label("Nome");
            ui.text_edit_singleline(&mut object.name);
            ui.end_row();
            ui.label("Imagem");
            ui.label(&object.image);
            ui.end_row();
            ui.label("Célula");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut object.x).prefix("x "));
                ui.add(egui::DragValue::new(&mut object.y).prefix("y "));
            });
            ui.end_row();
            ui.label("Ajuste (px)");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut object.offset[0]).prefix("x "));
                ui.add(egui::DragValue::new(&mut object.offset[1]).prefix("y "));
            });
            ui.end_row();
            // What a click on it does in the game.
            ui.label("Ao clicar");
            let label = |action: &Option<String>| match action.as_deref() {
                None => "Nada".to_owned(),
                Some("daily_rewards") => "Recompensas Diárias".to_owned(),
                Some(other) => other.to_owned(),
            };
            egui::ComboBox::from_id_salt("object-action")
                .selected_text(label(&object.action))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut object.action, None, "Nada");
                    ui.selectable_value(
                        &mut object.action,
                        Some("daily_rewards".to_owned()),
                        "Recompensas Diárias",
                    );
                });
            ui.end_row();
        });
        ui.label(format!("Bloqueia {} células", object.block.len()));
        ui.horizontal(|ui| {
            if ui.button("Nenhuma").clicked() {
                object.block.clear();
            }
            if ui.button("Só a base").clicked() {
                object.block = vec![[0, 0]];
            }
            if ui.button("3×3").clicked() {
                object.block = square(1);
            }
            if ui.button("5×5").clicked() {
                object.block = square(2);
            }
        });
        ui.small("Shift + clique no mapa liga/desliga uma célula.");
        let changed = *object != before;
        if ui.button("🗑 Remover objeto").clicked() {
            open.objects.objects.remove(index);
            self.selected = None;
            open.dirty = true;
            return;
        }
        if changed {
            open.dirty = true;
        }
    }

    fn import(&mut self, project: &mut Project, path: &std::path::Path, status: &mut String) {
        match import_object_image(&project.content, path, self.clear_background) {
            Ok(image) => {
                self.textures.loaded.remove(&format!("content:{image}"));
                *status = format!("Imagem salva em content/{image}. Clique no mapa para colocar.");
                self.placing = Some(image);
            }
            Err(error) => {
                *status = format!("Não foi possível importar {}: {error}", path.display())
            }
        }
    }

    fn view(&mut self, ui: &mut egui::Ui, project: &mut Project, status: &mut String) {
        let rect = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(rect, Sense::click_and_drag());
        let ctx = ui.ctx().clone();
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, Color32::from_gray(12));

        // Zoom around the cursor; pan with the right or middle button.
        if let Some(pointer) = response.hover_pos() {
            let scroll = ui.input(|input| input.smooth_scroll_delta.y);
            if scroll != 0.0 {
                let before = self.to_world(rect, pointer);
                self.zoom = (self.zoom * (1.0 + scroll * 0.0015)).clamp(0.05, 4.0);
                let after = self.to_world(rect, pointer);
                self.camera += before - after;
            }
        }
        if response.dragged_by(PointerButton::Secondary)
            || response.dragged_by(PointerButton::Middle)
        {
            self.camera -= response.drag_delta() / self.zoom;
        }
        self.keyboard(ui);

        let Some(open) = self.open.as_ref() else {
            return;
        };
        let visible = Rect::from_min_max(
            self.to_world_pos(rect, rect.min),
            self.to_world_pos(rect, rect.max),
        )
        .expand(64.0);

        // The map: background and scenery, as the client draws them.
        for sprite in &open.map.sprites {
            let background = sprite.depth == f32::NEG_INFINITY;
            if !background && !self.show_scenery {
                continue;
            }
            let origin = Pos2::new(sprite.origin[0], sprite.origin[1]);
            // Unknown sizes until decoded: tiles and most objects fit 512.
            let guess = Rect::from_min_size(origin, Vec2::splat(512.0));
            if !visible.intersects(guess) {
                continue;
            }
            let path = &sprite.frames[0];
            let texture = self.textures.get(&ctx, path, || {
                let bytes = project.installation.read(path).ok()?;
                image::load_from_memory(&bytes)
                    .ok()
                    .map(|image| image.to_rgba8())
            });
            if let Some((texture, size)) = texture {
                let world = Rect::from_min_size(origin, Vec2::new(size[0] as f32, size[1] as f32));
                if world.intersects(visible) {
                    painter.image(
                        texture.id(),
                        self.to_screen_rect(rect, world),
                        full_uv(),
                        Color32::WHITE,
                    );
                }
            }
        }

        // Cells.
        let (extent, height) = (open.map.extent, open.map.data.height);
        let cell_at = |zoomed: &Self, pos: Pos2| {
            let world = zoomed.to_world_pos(rect, pos);
            let cell = sandbox_data::map::unproject(extent, height, [world.x, world.y]);
            [cell[0].round() as i32, cell[1].round() as i32]
        };
        let ours_blocked = open
            .objects
            .objects
            .iter()
            .flat_map(MapObject::blocked_cells)
            .collect::<std::collections::HashSet<_>>();
        if self.show_cells {
            let corners = [
                rect.left_top(),
                rect.right_top(),
                rect.left_bottom(),
                rect.right_bottom(),
            ]
            .map(|p| cell_at(self, p));
            let min_x = corners.iter().map(|c| c[0]).min().unwrap_or(0).max(0);
            let max_x = corners
                .iter()
                .map(|c| c[0])
                .max()
                .unwrap_or(0)
                .min(open.map.data.width as i32 - 1);
            let min_y = corners.iter().map(|c| c[1]).min().unwrap_or(0).max(0);
            let max_y = corners
                .iter()
                .map(|c| c[1])
                .max()
                .unwrap_or(0)
                .min(open.map.data.height as i32 - 1);
            let count = i64::from(max_x - min_x + 1) * i64::from(max_y - min_y + 1);
            if count <= MAX_OVERLAY_CELLS {
                for y in min_y..=max_y {
                    for x in min_x..=max_x {
                        let blocked = open
                            .map
                            .data
                            .cell(x as u32, y as u32)
                            .is_some_and(|cell| cell.access != 0);
                        if blocked {
                            self.cell_shape(&painter, rect, open, [x, y], BLOCKED);
                        }
                    }
                }
            }
        }
        for &cell in &ours_blocked {
            self.cell_shape(&painter, rect, open, cell, OURS_BLOCKED);
        }
        if self.show_portals {
            for portal in &open.map.data.portals {
                let p = self.to_screen(rect, open.map.project([portal.x as f32, portal.y as f32]));
                painter.circle(
                    p,
                    10.0 * self.zoom.max(0.4),
                    Color32::from_rgba_premultiplied(40, 120, 255, 90),
                    Stroke::new(1.5, Color32::LIGHT_BLUE),
                );
            }
        }

        // Our objects, over everything.
        let mut object_rects = Vec::new();
        for (index, object) in open.objects.objects.iter().enumerate() {
            let key = format!("content:{}", object.image);
            let content = project.content.clone();
            let image = object.image.clone();
            let texture = self.textures.get(&ctx, &key, || {
                image::open(content.join(image))
                    .ok()
                    .map(|image| image.to_rgba8())
            });
            let p = open.map.project([object.x as f32, object.y as f32]);
            let (texture, size) = match texture {
                Some((texture, size)) => (Some(texture), size),
                None => (None, [48, 48]),
            };
            let offset = object.draw_offset(size);
            let world = Rect::from_min_size(
                Pos2::new(p[0] + offset[0] as f32, p[1] + offset[1] as f32),
                Vec2::new(size[0] as f32, size[1] as f32),
            );
            let screen = self.to_screen_rect(rect, world);
            match texture {
                Some(texture) => painter.image(texture.id(), screen, full_uv(), Color32::WHITE),
                None => painter.rect_filled(screen, 2.0, Color32::from_rgb(160, 40, 160)),
            };
            if self.selected == Some(index) {
                painter.rect_stroke(
                    screen,
                    0.0,
                    Stroke::new(2.0, SELECTED),
                    egui::StrokeKind::Outside,
                );
                self.cell_shape(
                    &painter,
                    rect,
                    open,
                    [object.x, object.y],
                    Color32::from_rgba_premultiplied(255, 210, 60, 70),
                );
            }
            object_rects.push((index, screen));
        }
        // The selected object's blocked cells, outlined over its image.
        if let Some(object) = self
            .selected
            .and_then(|index| open.objects.objects.get(index))
        {
            for cell in object.blocked_cells() {
                self.cell_outline(&painter, rect, open, cell, Stroke::new(1.5, OURS_OUTLINE));
            }
        }

        // Pointer: hover, select, move, place, block cells.
        self.hover = response.hover_pos().map(|pos| cell_at(self, pos));
        if let Some(cell) = self.hover {
            let info = open
                .map
                .data
                .cell(cell[0].max(0) as u32, cell[1].max(0) as u32)
                .map(|data| format!("acesso {} · altura {}", data.access, data.elevation))
                .unwrap_or_else(|| "fora do mapa".to_owned());
            painter.text(
                rect.left_bottom() + Vec2::new(8.0, -8.0),
                egui::Align2::LEFT_BOTTOM,
                format!(
                    "célula {}, {} · {info} · zoom {:.0}%",
                    cell[0],
                    cell[1],
                    self.zoom * 100.0
                ),
                egui::FontId::monospace(13.0),
                Color32::WHITE,
            );
            if let Some(image) = &self.placing {
                let key = format!("content:{image}");
                if let Some((texture, size)) = self.textures.loaded.get(&key).cloned().flatten() {
                    let object = MapObject {
                        image: image.clone(),
                        x: cell[0],
                        y: cell[1],
                        ..MapObject::default()
                    };
                    let p = open.map.project([cell[0] as f32, cell[1] as f32]);
                    let offset = object.draw_offset(size);
                    let world = Rect::from_min_size(
                        Pos2::new(p[0] + offset[0] as f32, p[1] + offset[1] as f32),
                        Vec2::new(size[0] as f32, size[1] as f32),
                    );
                    painter.image(
                        texture.id(),
                        self.to_screen_rect(rect, world),
                        full_uv(),
                        Color32::from_white_alpha(150),
                    );
                } else {
                    let content = project.content.clone();
                    let image = image.clone();
                    self.textures.get(&ctx, &key, || {
                        image::open(content.join(image))
                            .ok()
                            .map(|image| image.to_rgba8())
                    });
                }
            }
        }

        let dropped = ui.input(|input| {
            input
                .raw
                .dropped_files
                .first()
                .map(|file| file.path().to_path_buf())
        });
        if let Some(path) = dropped {
            self.import(project, &path, status);
        }

        let shift = ui.input(|input| input.modifiers.shift);
        let hit = |pos: Pos2| {
            object_rects
                .iter()
                .rev()
                .find(|(_, screen)| screen.contains(pos))
                .map(|(index, _)| *index)
        };
        if response.clicked_by(PointerButton::Primary)
            && let Some(pos) = response.interact_pointer_pos()
        {
            let cell = cell_at(self, pos);
            let open = self.open.as_mut().expect("open map");
            if let Some(image) = self.placing.take() {
                let name = std::path::Path::new(&image)
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned())
                    .unwrap_or_default();
                open.objects.objects.push(MapObject {
                    name,
                    image,
                    x: cell[0],
                    y: cell[1],
                    offset: [0, 0],
                    block: vec![[0, 0]],
                    action: None,
                });
                self.selected = Some(open.objects.objects.len() - 1);
                open.dirty = true;
                *status = format!(
                    "Objeto colocado em {}, {}. Salve o mapa para guardar.",
                    cell[0], cell[1]
                );
            } else if shift && let Some(index) = self.selected {
                let object = &mut open.objects.objects[index];
                let relative = [cell[0] - object.x, cell[1] - object.y];
                match object.block.iter().position(|block| *block == relative) {
                    Some(position) => {
                        object.block.remove(position);
                    }
                    None => object.block.push(relative),
                }
                open.dirty = true;
            } else {
                self.selected = hit(pos);
            }
        }
        if response.drag_started_by(PointerButton::Primary)
            && let Some(pos) = response.interact_pointer_pos()
            && let Some(index) = hit(pos)
        {
            self.selected = Some(index);
            let cell = cell_at(self, pos);
            let object = &self.open.as_ref().expect("open map").objects.objects[index];
            self.grab = Some([object.x - cell[0], object.y - cell[1]]);
        }
        if response.dragged_by(PointerButton::Primary)
            && let (Some(grab), Some(index), Some(pos)) =
                (self.grab, self.selected, response.interact_pointer_pos())
        {
            let cell = cell_at(self, pos);
            let open = self.open.as_mut().expect("open map");
            let object = &mut open.objects.objects[index];
            let target = [cell[0] + grab[0], cell[1] + grab[1]];
            if [object.x, object.y] != target {
                [object.x, object.y] = target;
                open.dirty = true;
            }
        }
        if response.drag_stopped() {
            self.grab = None;
        }
        if response.hovered()
            && ui.input(|input| input.key_pressed(Key::Delete))
            && let Some(index) = self.selected
        {
            let open = self.open.as_mut().expect("open map");
            open.objects.objects.remove(index);
            open.dirty = true;
            self.selected = None;
        }
        if self.placing.is_some() && ui.input(|input| input.key_pressed(Key::Escape)) {
            self.placing = None;
        }
    }

    /// Arrows (or WASD) walk the map, Shift faster; `+`/`-` zoom on the
    /// centre. Not while a text field takes the keys.
    fn keyboard(&mut self, ui: &egui::Ui) {
        if ui.ctx().egui_wants_keyboard_input() {
            return;
        }
        let (direction, fast, zoom_in, zoom_out, dt) = ui.input(|input| {
            let held = |keys: &[Key]| keys.iter().any(|key| input.key_down(*key));
            let mut direction = Vec2::ZERO;
            if held(&[Key::ArrowLeft, Key::A]) {
                direction.x -= 1.0;
            }
            if held(&[Key::ArrowRight, Key::D]) {
                direction.x += 1.0;
            }
            if held(&[Key::ArrowUp, Key::W]) {
                direction.y -= 1.0;
            }
            if held(&[Key::ArrowDown, Key::S]) {
                direction.y += 1.0;
            }
            (
                direction,
                input.modifiers.shift,
                input.key_pressed(Key::Plus) || input.key_pressed(Key::Equals),
                input.key_pressed(Key::Minus),
                input.stable_dt.min(0.1),
            )
        });
        if direction != Vec2::ZERO {
            // Screen pixels per second, whatever the zoom.
            let speed = if fast { 2400.0 } else { 800.0 };
            self.camera += direction.normalized() * speed * dt / self.zoom;
            ui.ctx().request_repaint();
        }
        if zoom_in || zoom_out {
            let factor = if zoom_in { 1.25 } else { 0.8 };
            self.zoom = (self.zoom * factor).clamp(0.05, 4.0);
        }
    }

    fn cell_outline(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        open: &OpenMap,
        cell: [i32; 2],
        stroke: Stroke,
    ) {
        let corner = |dx: f32, dy: f32| {
            self.to_screen(
                rect,
                open.map.project([cell[0] as f32 + dx, cell[1] as f32 + dy]),
            )
        };
        let points = vec![
            corner(-0.5, -0.5),
            corner(0.5, -0.5),
            corner(0.5, 0.5),
            corner(-0.5, 0.5),
        ];
        painter.add(egui::Shape::closed_line(points, stroke));
    }

    fn cell_shape(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        open: &OpenMap,
        cell: [i32; 2],
        colour: Color32,
    ) {
        let corner = |dx: f32, dy: f32| {
            self.to_screen(
                rect,
                open.map.project([cell[0] as f32 + dx, cell[1] as f32 + dy]),
            )
        };
        let points = vec![
            corner(-0.5, -0.5),
            corner(0.5, -0.5),
            corner(0.5, 0.5),
            corner(-0.5, 0.5),
        ];
        painter.add(egui::Shape::convex_polygon(points, colour, Stroke::NONE));
    }

    fn to_screen(&self, rect: Rect, world: [f32; 2]) -> Pos2 {
        rect.center() + (Vec2::new(world[0], world[1]) - self.camera) * self.zoom
    }

    fn to_screen_rect(&self, rect: Rect, world: Rect) -> Rect {
        Rect::from_min_max(
            self.to_screen(rect, [world.min.x, world.min.y]),
            self.to_screen(rect, [world.max.x, world.max.y]),
        )
    }

    fn to_world(&self, rect: Rect, screen: Pos2) -> Vec2 {
        (screen - rect.center()) / self.zoom + self.camera
    }

    fn to_world_pos(&self, rect: Rect, screen: Pos2) -> Pos2 {
        self.to_world(rect, screen).to_pos2()
    }
}

fn full_uv() -> Rect {
    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0))
}

/// The cells of a square of `radius` around the base.
fn square(radius: i32) -> Vec<[i32; 2]> {
    (-radius..=radius)
        .flat_map(|y| (-radius..=radius).map(move |x| [x, y]))
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn squares_cover_the_base() {
        assert_eq!(super::square(0), [[0, 0]]);
        assert_eq!(super::square(1).len(), 9);
        assert!(super::square(2).contains(&[-2, 2]));
    }
}
