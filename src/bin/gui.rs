use eframe::egui;
use mac_app_remover::*;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 620.0])
            .with_min_inner_size([700.0, 450.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Mac App Remover",
        options,
        Box::new(|_cc| Ok(Box::new(App::new()))),
    )
}

/// Arquivo residual com tamanho pre-calculado.
struct RelatedFile {
    path: PathBuf,
    size: u64,
}

/// Detalhes do app selecionado.
struct SelectedDetails {
    name: String,
    path: PathBuf,
    size: u64,
    bundle_id: Option<String>,
    related: Vec<RelatedFile>,
    total_size: u64,
}

struct App {
    /// Lista completa de apps (carregada uma vez, recarregavel).
    apps: Vec<AppInfo>,
    /// Texto da barra de busca.
    search_query: String,
    /// Indice do app selecionado na lista filtrada.
    selected_index: Option<usize>,
    /// Detalhes do app selecionado (carregados sob demanda).
    selected_details: Option<SelectedDetails>,
    /// Log de status das operacoes.
    log_messages: Vec<String>,
    /// Canal para receber mensagens de log da thread de remocao.
    log_rx: Option<mpsc::Receiver<LogMsg>>,
    /// Flag para indicar que a remocao esta em andamento.
    removing: bool,
    /// Flag para mostrar dialogo de confirmacao.
    show_confirm: bool,
}

enum LogMsg {
    Line(String),
    Done,
}

impl App {
    fn new() -> Self {
        let apps = get_installed_app_infos();
        Self {
            apps,
            search_query: String::new(),
            selected_index: None,
            selected_details: None,
            log_messages: Vec::new(),
            log_rx: None,
            removing: false,
            show_confirm: false,
        }
    }

    fn reload_apps(&mut self) {
        self.apps = get_installed_app_infos();
        self.selected_index = None;
        self.selected_details = None;
    }

    fn filtered_apps(&self) -> Vec<usize> {
        if self.search_query.is_empty() {
            return (0..self.apps.len()).collect();
        }
        let q = self.search_query.to_lowercase();
        self.apps
            .iter()
            .enumerate()
            .filter(|(_, app)| app.name.to_lowercase().contains(&q))
            .map(|(i, _)| i)
            .collect()
    }

    fn select_app(&mut self, global_index: usize) {
        let app = &self.apps[global_index];
        let related_paths = find_related_files(&app.name, app.bundle_id.as_deref());
        let mut total = app.size;
        let related: Vec<RelatedFile> = related_paths
            .into_iter()
            .map(|path| {
                let size = if path.is_dir() {
                    dir_size(&path).unwrap_or(0)
                } else {
                    std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0)
                };
                total += size;
                RelatedFile { path, size }
            })
            .collect();

        self.selected_details = Some(SelectedDetails {
            name: app.name.clone(),
            path: app.path.clone(),
            size: app.size,
            bundle_id: app.bundle_id.clone(),
            related,
            total_size: total,
        });
    }

    fn start_removal(&mut self) {
        let details = match &self.selected_details {
            Some(d) => d,
            None => return,
        };

        let app_path = details.path.clone();
        let app_name = details.name.clone();
        let related_paths: Vec<PathBuf> = details.related.iter().map(|r| r.path.clone()).collect();

        let (tx, rx) = mpsc::channel();
        self.log_rx = Some(rx);
        self.removing = true;
        self.log_messages.clear();
        self.show_confirm = false;

        thread::spawn(move || {
            // Verificar se o app esta em execucao e tentar fechar
            if is_app_running(&app_name) {
                let _ = tx.send(LogMsg::Line(format!(
                    "\"{}\" esta em execucao, tentando fechar...",
                    app_name
                )));
                quit_app(&app_name);
                thread::sleep(std::time::Duration::from_secs(2));
            }

            let _ = tx.send(LogMsg::Line(format!(
                "Removendo {}...",
                app_path.display()
            )));
            match remove_path(&app_path) {
                Ok(_) => {
                    let _ = tx.send(LogMsg::Line(format!(
                        "  {} - OK",
                        app_path.display()
                    )));
                }
                Err(e) => {
                    let _ = tx.send(LogMsg::Line(format!(
                        "  {} - ERRO: {}",
                        app_path.display(),
                        e
                    )));
                }
            }

            for path in &related_paths {
                let _ = tx.send(LogMsg::Line(format!("Removendo {}...", path.display())));
                match remove_path(path) {
                    Ok(_) => {
                        let _ = tx.send(LogMsg::Line(format!("  {} - OK", path.display())));
                    }
                    Err(e) => {
                        let _ = tx.send(LogMsg::Line(format!(
                            "  {} - ERRO: {}",
                            path.display(),
                            e
                        )));
                    }
                }
            }

            let _ = tx.send(LogMsg::Line(format!(
                "\n\"{}\" removido com sucesso!",
                app_name
            )));
            let _ = tx.send(LogMsg::Done);
        });
    }

    fn poll_log(&mut self) {
        let done = if let Some(rx) = &self.log_rx {
            let mut finished = false;
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    LogMsg::Line(line) => self.log_messages.push(line),
                    LogMsg::Done => {
                        finished = true;
                    }
                }
            }
            finished
        } else {
            false
        };

        if done {
            self.removing = false;
            self.log_rx = None;
            self.reload_apps();
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_log();

        // Solicitar repaint enquanto estiver removendo para atualizar o log.
        if self.removing {
            ctx.request_repaint();
        }

        // Painel superior: barra de busca
        egui::TopBottomPanel::top("search_bar").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label("Buscar:");
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.search_query)
                        .desired_width(300.0)
                        .hint_text("Filtrar aplicativos..."),
                );
                if response.changed() {
                    self.selected_index = None;
                    self.selected_details = None;
                }
                if ui.button("Recarregar").clicked() {
                    self.reload_apps();
                }
                ui.label(format!("{} apps", self.apps.len()));
            });
            ui.add_space(4.0);
        });

        // Painel inferior: log de status
        egui::TopBottomPanel::bottom("log_panel")
            .min_height(100.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Log").strong());
                ui.separator();
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for msg in &self.log_messages {
                            ui.monospace(msg);
                        }
                        if self.removing {
                            ui.spinner();
                        }
                    });
            });

        // Painel direito: detalhes do app selecionado
        egui::SidePanel::right("details_panel")
            .min_width(320.0)
            .default_width(380.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                if let Some(details) = &self.selected_details {
                    ui.heading(&details.name);
                    ui.add_space(4.0);

                    egui::Grid::new("app_details_grid")
                        .num_columns(2)
                        .spacing([8.0, 4.0])
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new("Caminho:").strong());
                            ui.label(details.path.display().to_string());
                            ui.end_row();

                            if let Some(ref bid) = details.bundle_id {
                                ui.label(egui::RichText::new("Bundle ID:").strong());
                                ui.label(bid);
                                ui.end_row();
                            }

                            ui.label(egui::RichText::new("Tamanho:").strong());
                            ui.label(format_size(details.size));
                            ui.end_row();
                        });

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);

                    if details.related.is_empty() {
                        ui.label("Nenhum arquivo residual encontrado.");
                    } else {
                        ui.label(
                            egui::RichText::new(format!(
                                "Arquivos residuais ({}):",
                                details.related.len()
                            ))
                            .strong(),
                        );
                        ui.add_space(4.0);
                        egui::ScrollArea::vertical()
                            .max_height(200.0)
                            .show(ui, |ui| {
                                for rf in &details.related {
                                    ui.horizontal(|ui| {
                                        ui.monospace(format!(
                                            "{} ({})",
                                            rf.path.display(),
                                            format_size(rf.size)
                                        ));
                                    });
                                }
                            });
                    }

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);

                    ui.label(
                        egui::RichText::new(format!(
                            "Total a liberar: {}",
                            format_size(details.total_size)
                        ))
                        .strong()
                        .size(15.0),
                    );

                    ui.add_space(12.0);

                    // Botao de remover
                    let can_remove = !self.removing;
                    ui.add_enabled_ui(can_remove, |ui| {
                        if ui
                            .button(
                                egui::RichText::new("Remover aplicativo")
                                    .size(16.0)
                                    .color(egui::Color32::WHITE),
                            )
                            .clicked()
                        {
                            self.show_confirm = true;
                        }
                    });

                } else {
                    ui.vertical_centered(|ui| {
                        ui.add_space(80.0);
                        ui.label(
                            egui::RichText::new("Selecione um aplicativo na lista")
                                .size(14.0)
                                .weak(),
                        );
                    });
                }
            });

        // Dialogo de confirmacao (fora do side panel para evitar conflito de borrow)
        if self.show_confirm {
            let (confirm_name, confirm_size) = self
                .selected_details
                .as_ref()
                .map(|d| (d.name.clone(), d.total_size))
                .unwrap_or_default();

            egui::Window::new("Confirmar remocao")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(format!(
                        "Tem certeza que deseja remover \"{}\"?",
                        confirm_name
                    ));
                    ui.label(format!(
                        "Isso ira liberar {}.",
                        format_size(confirm_size)
                    ));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Cancelar").clicked() {
                            self.show_confirm = false;
                        }
                        if ui
                            .button(egui::RichText::new("Confirmar").strong())
                            .clicked()
                        {
                            self.start_removal();
                        }
                    });
                });
        }

        // Painel central: lista de apps
        egui::CentralPanel::default().show(ctx, |ui| {
            let filtered = self.filtered_apps();

            if filtered.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label("Nenhum aplicativo encontrado.");
                });
                return;
            }

            egui::ScrollArea::vertical().show(ui, |ui| {
                for (list_pos, &global_idx) in filtered.iter().enumerate() {
                    let app = &self.apps[global_idx];
                    let is_selected = self.selected_index == Some(list_pos);

                    let response = ui.selectable_label(
                        is_selected,
                        format!("{}    {}", app.name, format_size(app.size)),
                    );

                    if response.clicked() {
                        self.selected_index = Some(list_pos);
                        self.select_app(global_idx);
                    }
                }
            });
        });
    }
}
