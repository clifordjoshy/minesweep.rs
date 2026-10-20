use egui::{Align2, Color32, FontId, Rect, Sense, Vec2, vec2};

use crate::app;
use crate::config;
use minesweeper_core::{CellContent, CellState};

fn number_color(n: u8) -> Color32 {
    match n {
        1 => Color32::from_rgb(0, 0, 255),
        2 => Color32::from_rgb(0, 128, 0),
        3 => Color32::from_rgb(255, 0, 0),
        4 => Color32::from_rgb(0, 0, 128),
        5 => Color32::from_rgb(128, 0, 0),
        6 => Color32::from_rgb(0, 128, 128),
        7 => Color32::BLACK,
        _ => Color32::GRAY,
    }
}

/// Paint a single cell: background colour plus an optional glyph.
fn paint_cell(ui: &egui::Ui, rect: Rect, state: CellState, hovered: bool) {
    let painter = ui.painter();

    let (bg, glyph): (Color32, Option<(String, Color32)>) = match state {
        CellState::Hidden => {
            let bg = if hovered {
                Color32::from_gray(150)
            } else {
                Color32::from_gray(120)
            };
            (bg, None)
        }
        CellState::Flagged => (
            Color32::from_gray(120),
            Some(("F".to_owned(), Color32::from_rgb(220, 30, 30))),
        ),
        CellState::Revealed(content) => {
            let revealed_bg = Color32::from_gray(210);
            match content {
                CellContent::Empty => (revealed_bg, None),
                CellContent::Number(n) => (revealed_bg, Some((n.to_string(), number_color(n)))),
                CellContent::Mine => (
                    Color32::from_rgb(200, 60, 60),
                    Some(("*".to_owned(), Color32::BLACK)),
                ),
            }
        }
    };

    painter.rect_filled(rect, 2.0, bg);

    if let Some((text, color)) = glyph {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            text,
            FontId::monospace(config::CELL_SIZE * 0.6),
            color,
        );
    }
}

impl eframe::App for app::App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("top_panel").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                // NOTE: no File->Quit on web pages!
                let is_web = cfg!(target_arch = "wasm32");
                if !is_web {
                    ui.menu_button("File", |ui| {
                        if ui.button("Quit").clicked() {
                            ui.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                    ui.add_space(16.0);
                }

                egui::widgets::global_theme_preference_buttons(ui);
            });
        });

        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading(config::APP_NAME);

            // --- Controls: difficulty picker + new game button -------------
            ui.horizontal(|ui| {
                let mut difficulty_changed = false;
                for d in config::Difficulty::ALL {
                    if ui
                        .selectable_value(&mut self.difficulty, d, d.label())
                        .changed()
                    {
                        difficulty_changed = true;
                    }
                }
                if difficulty_changed {
                    self.reset();
                }

                ui.add_space(16.0);
                if ui.button("New game").clicked() {
                    self.reset();
                }
            });

            // --- Status line ------------------------------------------------
            ui.horizontal(|ui| {
                ui.label(format!(
                    "Mines left: {}/{}",
                    self.mines_remaining(),
                    self.difficulty.params().2
                ));
                ui.separator();
                ui.label(self.status.label());
            });

            ui.separator();

            // --- The grid ---------------------------------------------------
            let (w, h, _) = self.difficulty.params();

            // Reserve one rectangle for the whole board, then carve cells out of it.
            let board_size = vec2(w as f32 * config::CELL_SIZE, h as f32 * config::CELL_SIZE);
            let (board_rect, _) = ui.allocate_exact_size(board_size, Sense::hover());

            for idx in 0..w * h {
                let (x, y) = (idx % w, idx / w);

                let top_left = board_rect.min
                    + vec2(x as f32 * config::CELL_SIZE, y as f32 * config::CELL_SIZE);
                // shrink() leaves a thin gap between cells, which acts as the grid lines.
                let cell_rect =
                    Rect::from_min_size(top_left, Vec2::splat(config::CELL_SIZE)).shrink(0.5);

                // Each cell gets its own interaction response (needs a unique id).
                let response =
                    ui.interact(cell_rect, ui.scope_id().with(("cell", idx)), Sense::click());

                // Before the first click there is no board, so everything is hidden.
                let state = match &self.board {
                    Some(board) => board.state_grid[idx],
                    None => CellState::Hidden,
                };

                paint_cell(ui, cell_rect, state, response.hovered());

                if response.clicked() {
                    self.on_cell_click(idx);
                } else if response.secondary_clicked() {
                    self.on_cell_flagged(idx);
                }
            }

            ui.horizontal(|ui| {
                ui.label(format!("Seed: {}", self.get_seed()));
                ui.separator();
                ui.label(self.status.label());
            });

            egui::warn_if_debug_build(ui);
        });
    }

    /// Called by the framework to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }
}
