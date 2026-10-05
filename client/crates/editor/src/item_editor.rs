//! The item editor: search the items, change their columns and icon, create
//! new ones from a base.

use crate::project::{Project, import_icon};
use eframe::egui::{self, Color32, RichText, TextureHandle, TextureOptions};
use gravak_content::items::{ColumnValue, ItemDef};
use gravak_content::itemtype::{COLUMNS, ColumnKind, ItemType};
use std::collections::HashMap;

/// The columns in groups, as the form shows them.
const GROUPS: [(&str, &[usize]); 5] = [
    ("Geral", &[1, 37, 38, 12, 36, 11, 10, 13]),
    ("Requisitos", &[2, 3, 4, 5, 6, 7, 8, 9]),
    ("Combate", &[14, 15, 16, 17, 18, 29, 30, 31, 32, 19, 20]),
    ("Durabilidade", &[21, 22, 33, 34]),
    ("Avançado", &[23, 24, 25, 26, 27, 28, 35]),
];

const CHANGED: Color32 = Color32::from_rgb(230, 180, 60);

/// The item being edited.
struct Draft {
    id: u32,
    base: Option<u32>,
    icon: Option<String>,
    /// The item before our definition: what an unchanged field goes back to.
    underlying: ItemType,
    values: Vec<String>,
    dirty: bool,
}

impl Draft {
    fn new(project: &Project, id: u32) -> Self {
        let def = project.def(id);
        let base = def.and_then(|def| def.base);
        let underlying = project.underlying(id, base);
        let item = project
            .item(id)
            .cloned()
            .unwrap_or_else(|| underlying.clone());
        Self {
            id,
            base,
            icon: def.and_then(|def| def.icon.clone()),
            values: (0..COLUMNS.len())
                .map(|column| item.value(column))
                .collect(),
            underlying,
            dirty: false,
        }
    }

    fn changed(&self, column: usize) -> bool {
        self.values[column] != self.underlying.value(column)
    }

    /// The definition this draft saves: the base, the icon and the columns
    /// that differ from the item underneath.
    fn to_def(&self) -> Result<ItemDef, String> {
        let mut def = ItemDef {
            id: self.id,
            base: self.base,
            icon: self.icon.clone(),
            values: Vec::new(),
        };
        for (column, spec) in COLUMNS.iter().enumerate().skip(1) {
            if !self.changed(column) {
                continue;
            }
            let text = self.values[column].trim();
            let value =
                match spec.kind {
                    ColumnKind::Number => ColumnValue::Number(text.parse().map_err(|_| {
                        format!("{}: \"{text}\" não é um número inteiro", spec.label)
                    })?),
                    ColumnKind::Text => ColumnValue::Text(text.to_owned()),
                };
            def.set(column, Some(value));
        }
        Ok(def)
    }
}

struct NewItem {
    base: String,
    id: String,
    name: String,
}

#[derive(Default)]
pub struct ItemEditor {
    search: String,
    only_ours: bool,
    selected: Option<u32>,
    draft: Option<Draft>,
    new_item: Option<NewItem>,
    icon_path: String,
    clear_white: bool,
    icons: HashMap<u32, Option<TextureHandle>>,
}

impl ItemEditor {
    pub fn new() -> Self {
        Self {
            clear_white: true,
            ..Self::default()
        }
    }

    /// Forgets what was read from the project (after a reload).
    pub fn refresh(&mut self, project: &Project) {
        self.icons.clear();
        if let Some(id) = self.selected {
            if project.item(id).is_some() {
                self.draft = Some(Draft::new(project, id));
            } else {
                self.selected = None;
                self.draft = None;
            }
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, project: &mut Project, status: &mut String) {
        self.dropped_files(ui, project, status);
        egui::Panel::left("item-list")
            .resizable(true)
            .default_size(320.0)
            .show(ui, |ui| {
                self.list(ui, project);
            });
        egui::CentralPanel::default_margins().show(ui, |ui| match self.draft.is_some() {
            true => self.form(ui, project, status),
            false => {
                ui.centered_and_justified(|ui| {
                    ui.label("Escolha um item na lista ou crie um novo.")
                });
            }
        });
        self.new_item_window(ui, project, status);
    }

    fn select(&mut self, project: &Project, id: u32) {
        self.selected = Some(id);
        self.draft = Some(Draft::new(project, id));
        self.icon_path.clear();
    }

    fn list(&mut self, ui: &mut egui::Ui, project: &mut Project) {
        ui.horizontal(|ui| {
            ui.label("Buscar");
            ui.text_edit_singleline(&mut self.search);
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.only_ours, "Só os nossos");
            if ui.button("➕ Novo item").clicked() {
                let base = self.selected.unwrap_or(0);
                self.new_item = Some(NewItem {
                    base: if base == 0 {
                        String::new()
                    } else {
                        base.to_string()
                    },
                    id: String::new(),
                    name: String::new(),
                });
            }
        });
        ui.separator();
        let filter = self.search.trim().to_lowercase();
        let rows = project
            .merged
            .merged
            .items()
            .filter(|item| !self.only_ours || project.def(item.id()).is_some())
            .map(|item| (item.id(), item.name()))
            .filter(|(id, name)| {
                filter.is_empty()
                    || name.to_lowercase().contains(&filter)
                    || id.to_string().contains(&filter)
            })
            .collect::<Vec<_>>();
        ui.label(format!("{} itens", rows.len()));
        let row_height = ui.spacing().interact_size.y;
        egui::ScrollArea::vertical().auto_shrink(false).show_rows(
            ui,
            row_height,
            rows.len(),
            |ui, range| {
                for (id, name) in &rows[range] {
                    let ours = project.def(*id).is_some();
                    let mut text = RichText::new(format!("{id:>8}  {name}")).monospace();
                    if ours {
                        text = text.color(CHANGED);
                    }
                    if ui
                        .selectable_label(self.selected == Some(*id), text)
                        .clicked()
                    {
                        self.select(project, *id);
                    }
                }
            },
        );
    }

    fn form(&mut self, ui: &mut egui::Ui, project: &mut Project, status: &mut String) {
        let id = self
            .draft
            .as_ref()
            .map(|draft| draft.id)
            .unwrap_or_default();
        ui.horizontal(|ui| {
            self.icon_box(ui, project, id, status);
            ui.vertical(|ui| {
                let draft = self.draft.as_ref().expect("a draft");
                ui.heading(format!("{}  {}", draft.id, draft.values[1]));
                let origin = match (
                    draft.base,
                    project.original().contains(draft.id),
                    project.def(draft.id),
                ) {
                    (Some(base), _, _) => format!("Item nosso, criado a partir do {base}"),
                    (None, true, Some(_)) => "Item original com alterações nossas".to_owned(),
                    (None, true, None) => "Item original".to_owned(),
                    (None, false, _) => "Item nosso".to_owned(),
                };
                ui.label(origin);
                ui.horizontal(|ui| {
                    let dirty = self.draft.as_ref().is_some_and(|draft| draft.dirty);
                    if ui
                        .add_enabled(dirty, egui::Button::new("💾 Salvar"))
                        .clicked()
                    {
                        self.save(project, status);
                    }
                    if ui
                        .add_enabled(dirty, egui::Button::new("Descartar"))
                        .clicked()
                    {
                        self.draft = Some(Draft::new(project, id));
                    }
                    if ui.button("Duplicar").clicked() {
                        self.new_item = Some(NewItem {
                            base: id.to_string(),
                            id: String::new(),
                            name: String::new(),
                        });
                    }
                    if project.def(id).is_some() && ui.button("🗑 Remover customização").clicked()
                    {
                        self.remove(project, id, status);
                    }
                });
            });
        });
        ui.separator();
        let Some(draft) = self.draft.as_mut() else {
            return;
        };
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                for (group, columns) in GROUPS {
                    egui::CollapsingHeader::new(group)
                        .default_open(group != "Avançado")
                        .show(ui, |ui| {
                            egui::Grid::new(group)
                                .num_columns(3)
                                .striped(true)
                                .show(ui, |ui| {
                                    for &column in columns {
                                        field(ui, draft, column);
                                        ui.end_row();
                                    }
                                });
                        });
                }
                let item = project.item(draft.id);
                if let Some(item) = item.filter(|item| item.len() > COLUMNS.len()) {
                    egui::CollapsingHeader::new("Colunas extras (só leitura)").show(ui, |ui| {
                        for column in COLUMNS.len()..item.len() {
                            ui.label(format!("{column}: {}", item.text(column)));
                        }
                    });
                }
            });
    }

    fn icon_box(&mut self, ui: &mut egui::Ui, project: &mut Project, id: u32, status: &mut String) {
        ui.vertical(|ui| {
            let texture = self
                .icons
                .entry(id)
                .or_insert_with(|| {
                    let draft_icon = self.draft.as_ref().and_then(|draft| draft.icon.clone());
                    project.icon(id, draft_icon.as_deref()).map(|image| {
                        let size = [image.width() as usize, image.height() as usize];
                        let image = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
                        ui.ctx()
                            .load_texture(format!("icon-{id}"), image, TextureOptions::NEAREST)
                    })
                })
                .clone();
            let (rect, _) = ui.allocate_exact_size(egui::vec2(64.0, 64.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 4.0, Color32::from_gray(30));
            match texture {
                Some(texture) => {
                    let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                    ui.painter().image(texture.id(), rect, uv, Color32::WHITE);
                }
                None => {
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "sem ícone",
                        egui::FontId::proportional(11.0),
                        Color32::GRAY,
                    );
                }
            }
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.icon_path)
                        .hint_text("PNG/JPG/BMP…")
                        .desired_width(150.0),
                );
                if ui.button("Importar").clicked() {
                    let path = std::path::PathBuf::from(self.icon_path.trim().trim_matches('"'));
                    self.import_icon(project, &path, status);
                }
            });
            ui.checkbox(&mut self.clear_white, "Tirar o fundo da imagem");
            ui.small("ou arraste a imagem para a janela");
        });
    }

    fn dropped_files(&mut self, ui: &mut egui::Ui, project: &mut Project, status: &mut String) {
        let dropped = ui.input(|input| input.raw.dropped_files.clone());
        let Some(path) = dropped.first().map(|file| file.path().to_path_buf()) else {
            return;
        };
        if self.draft.is_none() {
            *status = "Escolha um item antes de soltar o ícone.".to_owned();
            return;
        }
        self.import_icon(project, &path, status);
    }

    /// Copies the image into `content/icons/<id>.png`, already 64x64, and
    /// names it in the draft (saved with the item).
    fn import_icon(&mut self, project: &mut Project, path: &std::path::Path, status: &mut String) {
        let Some(draft) = self.draft.as_mut() else {
            return;
        };
        match import_icon(&project.content, draft.id, path, self.clear_white) {
            Ok(relative) => {
                draft.icon = Some(relative.clone());
                draft.dirty = true;
                self.icons.remove(&draft.id);
                *status = format!("Ícone salvo em content/{relative}; salve o item para usá-lo.");
            }
            Err(error) => {
                *status = format!("Não foi possível importar {}: {error}", path.display())
            }
        }
    }

    fn save(&mut self, project: &mut Project, status: &mut String) {
        let Some(draft) = self.draft.as_ref() else {
            return;
        };
        let def = match draft.to_def() {
            Ok(def) => def,
            Err(error) => {
                *status = error;
                return;
            }
        };
        let id = def.id;
        let result = project
            .merged
            .items
            .save(&project.content, def)
            .and_then(|path| project.reload().map(|()| path));
        match result {
            Ok(path) => {
                *status = format!(
                    "Item {id} salvo em {}. Rode o Build para o jogo usar.",
                    path.display()
                );
                self.icons.remove(&id);
                self.draft = Some(Draft::new(project, id));
            }
            Err(error) => *status = format!("Erro ao salvar: {error}"),
        }
    }

    fn remove(&mut self, project: &mut Project, id: u32, status: &mut String) {
        let result = project
            .merged
            .items
            .remove(id)
            .and_then(|()| project.reload());
        match result {
            Ok(()) => {
                *status = format!("Customização do item {id} removida.");
                self.icons.remove(&id);
                if project.item(id).is_some() {
                    self.select(project, id);
                } else {
                    self.selected = None;
                    self.draft = None;
                }
            }
            Err(error) => *status = format!("Erro ao remover: {error}"),
        }
    }

    fn new_item_window(&mut self, ui: &mut egui::Ui, project: &mut Project, status: &mut String) {
        let Some(mut new_item) = self.new_item.take() else {
            return;
        };
        let mut open = true;
        let mut create = false;
        egui::Window::new("Novo item").collapsible(false).resizable(false).open(&mut open).show(ui.ctx(), |ui| {
            egui::Grid::new("new-item").num_columns(2).show(ui, |ui| {
                ui.label("Copiar do item (base)");
                ui.text_edit_singleline(&mut new_item.base);
                ui.end_row();
                let base = new_item.base.trim().parse::<u32>().ok();
                let base_item = base.and_then(|base| project.item(base));
                ui.label("");
                ui.label(match base_item {
                    Some(item) => RichText::new(item.name()).color(Color32::LIGHT_GREEN),
                    None if new_item.base.trim().is_empty() => RichText::new("vazio: todas as colunas 0"),
                    None => RichText::new("esse item não existe").color(Color32::LIGHT_RED),
                });
                ui.end_row();
                ui.label("Novo ID");
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut new_item.id);
                    if let Some(base) = base
                        && ui.button("Sugerir").clicked()
                    {
                        let taken = |id| project.def(id).is_some();
                        if let Some(id) = project.merged.merged.suggest_id(base, taken) {
                            new_item.id = id.to_string();
                        }
                    }
                });
                ui.end_row();
                ui.label("Nome");
                ui.text_edit_singleline(&mut new_item.name);
                ui.end_row();
            });
            ui.small("O ID define a família: o slot de equipamento vem dos milhares (410xxx arma de uma mão, 130xxx armadura…) e o último dígito é a qualidade.");
            if ui.button("Criar").clicked() {
                create = true;
            }
        });
        if create {
            match self.create(project, &new_item) {
                Ok(id) => {
                    *status = format!("Item {id} criado.");
                    return;
                }
                Err(error) => *status = error,
            }
        }
        if open {
            self.new_item = Some(new_item);
        }
    }

    fn create(&mut self, project: &mut Project, new_item: &NewItem) -> Result<u32, String> {
        let id = new_item
            .id
            .trim()
            .parse::<u32>()
            .map_err(|_| "Novo ID inválido.".to_owned())?;
        if project.item(id).is_some() {
            return Err(format!(
                "O ID {id} já existe; escolha outro (ou edite o existente)."
            ));
        }
        let base = match new_item.base.trim() {
            "" => None,
            base => {
                let base = base
                    .parse::<u32>()
                    .map_err(|_| "Base inválida.".to_owned())?;
                project
                    .item(base)
                    .ok_or(format!("A base {base} não existe."))?;
                Some(base)
            }
        };
        let mut def = ItemDef {
            id,
            base,
            ..ItemDef::default()
        };
        let name = new_item.name.trim();
        if !name.is_empty() {
            def.set(1, Some(ColumnValue::Text(name.to_owned())));
        }
        project
            .merged
            .items
            .save(&project.content, def)
            .map_err(|error| error.to_string())?;
        project.reload().map_err(|error| error.to_string())?;
        self.select(project, id);
        Ok(id)
    }
}

fn field(ui: &mut egui::Ui, draft: &mut Draft, column: usize) {
    let spec = COLUMNS[column];
    let changed = draft.changed(column);
    let label = RichText::new(spec.label);
    ui.label(if changed { label.color(CHANGED) } else { label })
        .on_hover_text(format!(
            "coluna {column} · `{}` · original: {}",
            spec.key,
            draft.underlying.value(column)
        ));
    let width = if spec.kind == ColumnKind::Text {
        320.0
    } else {
        120.0
    };
    let response = ui.add_sized(
        [width, ui.spacing().interact_size.y],
        egui::TextEdit::singleline(&mut draft.values[column]),
    );
    if response.changed() {
        draft.dirty = true;
    }
    if changed {
        if ui
            .small_button("↺")
            .on_hover_text("Voltar ao valor original")
            .clicked()
        {
            draft.values[column] = draft.underlying.value(column);
            draft.dirty = true;
        }
    } else {
        ui.label("");
    }
}
