//! Gravak Editor: the content of the game (items now, maps next) edited as
//! files in `content/`, built over the 5065 installation.

mod item_editor;
mod project;

use eframe::egui;
use gravak_content::build::{build, default_content_dir, default_installation};
use item_editor::ItemEditor;
use project::Project;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Settings {
    installation: String,
    content: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            installation: default_installation().display().to_string(),
            content: default_content_dir().display().to_string(),
        }
    }
}

struct EditorApp {
    settings: Settings,
    project: Option<Project>,
    items: ItemEditor,
    status: String,
}

impl EditorApp {
    fn new(context: &eframe::CreationContext<'_>) -> Self {
        let settings = context
            .storage
            .and_then(|storage| eframe::get_value(storage, "settings"))
            .unwrap_or_default();
        let mut app = Self {
            settings,
            project: None,
            items: ItemEditor::new(),
            status: String::new(),
        };
        app.open();
        app
    }

    fn open(&mut self) {
        let installation = std::path::PathBuf::from(self.settings.installation.trim());
        let content = std::path::PathBuf::from(self.settings.content.trim());
        match Project::open(&installation, &content) {
            Ok(project) => {
                self.status = format!(
                    "{} itens da instalação, {} definições nossas em {}",
                    project.original().item_count(),
                    project.merged.items.defs().count(),
                    content.display()
                );
                self.items.refresh(&project);
                self.project = Some(project);
            }
            Err(error) => {
                self.project = None;
                self.status = format!("Não foi possível abrir: {error}");
            }
        }
    }

    fn build(&mut self) {
        let Some(project) = self.project.as_mut() else {
            return;
        };
        self.status = match build(&mut project.installation, &project.content) {
            Ok(report) => format!(
                "Build: {} itens novos, {} alterados, {} ícones em {}. Reinicie cliente e servidor.",
                report.added,
                report.changed,
                report.icons,
                report.output.display()
            ),
            Err(error) => format!("Build falhou: {error}"),
        };
    }
}

impl eframe::App for EditorApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("paths").show(ui, |ui| {
            ui.add_space(4.0);
            egui::Grid::new("paths-grid").num_columns(2).show(ui, |ui| {
                ui.label("Instalação 5065");
                ui.add(egui::TextEdit::singleline(&mut self.settings.installation).desired_width(420.0));
                ui.end_row();
                ui.label("Pasta content");
                ui.add(egui::TextEdit::singleline(&mut self.settings.content).desired_width(420.0));
                ui.end_row();
            });
            ui.horizontal(|ui| {
                if ui.button("📂 Abrir / Recarregar").clicked() {
                    self.open();
                }
                let can_build = self.project.is_some();
                if ui
                    .add_enabled(can_build, egui::Button::new("🔨 Build"))
                    .on_hover_text("Gera content/build: o itemtype.dat e os ícones que o cliente e o servidor usam")
                    .clicked()
                {
                    self.build();
                }
            });
            ui.add_space(4.0);
        });
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.label(&self.status);
        });
        match self.project.as_mut() {
            Some(project) => self.items.ui(ui, project, &mut self.status),
            None => {
                egui::CentralPanel::default_margins().show(ui, |ui| {
                    ui.centered_and_justified(|ui| {
                        ui.label("Informe a pasta da instalação 5065 e a pasta content do repositório, e clique em Abrir.");
                    });
                });
            }
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "settings", &self.settings);
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Gravak Editor")
            .with_inner_size([1200.0, 800.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native(
        "Gravak Editor",
        options,
        Box::new(|context| Ok(Box::new(EditorApp::new(context)))),
    )
}
