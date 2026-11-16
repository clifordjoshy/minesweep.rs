use crate::app;
use crate::config;
use egui::{Align2, Color32, FontId, Painter, Rect, Sense, Shape, Stroke, Vec2, vec2};
use minesweeper_core::{CellContent, CellState};

enum Glyph {
    Text(String, Color32),
    Flag,
}

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
fn paint_flag(painter: &Painter, rect: Rect) {
    // map fractional (0..1) coordinates in the cell to screen positions.
    let p = |fx: f32, fy: f32| rect.min + vec2(fx * rect.width(), fy * rect.height());
    let pole_color = Color32::from_gray(25);
    let pole_w = rect.width() * 0.07;

    // pole
    painter.line_segment(
        [p(0.52, 0.20), p(0.52, 0.78)],
        Stroke::new(pole_w, pole_color),
    );

    // base
    painter.rect_filled(
        Rect::from_min_max(p(0.35, 0.78), p(0.68, 0.86)),
        1.0,
        pole_color,
    );

    // cloth
    painter.add(Shape::convex_polygon(
        vec![p(0.56, 0.16), p(0.56, 0.50), p(0.18, 0.35), p(0.18, 0.31)],
        Color32::from_rgb(195, 19, 19),
        Stroke::NONE,
    ));
}

/// Paint a single cell: background colour plus an optional glyph.
fn paint_cell(
    ui: &egui::Ui,
    rect: Rect,
    state: CellState,
    incorrectly_marked: bool,
    hovered: bool,
) {
    let painter = ui.painter();

    let (bg, glyph): (Color32, Option<Glyph>) = match state {
        CellState::Hidden => {
            let bg = if hovered {
                Color32::from_gray(150)
            } else {
                Color32::from_gray(120)
            };
            (bg, None)
        }
        CellState::Flagged => (Color32::from_gray(120), Some(Glyph::Flag)),
        CellState::Revealed(content) => {
            let revealed_bg = Color32::from_gray(210);
            match content {
                CellContent::Empty => (revealed_bg, None),
                CellContent::Number(n) if incorrectly_marked => (
                    Color32::from_rgb(255, 195, 195),
                    Some(Glyph::Text(n.to_string(), number_color(n))),
                ),
                CellContent::Number(n) => (
                    revealed_bg,
                    Some(Glyph::Text(n.to_string(), number_color(n))),
                ),
                CellContent::Mine => (
                    Color32::from_rgb(200, 60, 60),
                    Some(Glyph::Text("*".to_owned(), Color32::BLACK)),
                ),
            }
        }
    };

    painter.rect_filled(rect, 2.0, bg);

    match glyph {
        Some(Glyph::Text(text, color)) => {
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                text,
                FontId::monospace(config::CELL_SIZE * 0.6),
                color,
            );
        }
        Some(Glyph::Flag) => paint_flag(painter, rect),
        None => {}
    }
}

impl eframe::App for app::App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                if let Some(icon) = &self.icon {
                    ui.add(egui::Image::new(icon).max_height(18.0));
                }
                ui.heading(config::APP_NAME);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    egui::widgets::global_theme_preference_buttons(ui);
                });
            });

            // --- Controls: difficulty picker -------------
            ui.horizontal_wrapped(|ui| {
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
            });

            // --- Controls: Board Type  + new game button -------------
            ui.horizontal_wrapped(|ui| {
                let mut changed = false;
                for b in config::BoardType::ALL {
                    if ui
                        .selectable_value(&mut self.board_type, b, b.label())
                        .changed()
                    {
                        changed = true;
                    }
                }
                if changed {
                    self.reset();
                }

                ui.separator();
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;

                    if ui
                        .add(
                            egui::Button::new("New Game").corner_radius(egui::CornerRadius {
                                nw: 4,
                                ne: 0,
                                sw: 4,
                                se: 0,
                            }),
                        )
                        .clicked()
                    {
                        self.reset();
                    }
                    ui.add_space(2.0);
                    let seed_id = egui::Id::unique("seed_input");
                    if ui
                        .add(
                            egui::Button::new(if self.seed_mode { "◂" } else { "▸" })
                                .corner_radius(egui::CornerRadius {
                                    nw: 0,
                                    ne: 4,
                                    sw: 0,
                                    se: 4,
                                }),
                        )
                        .clicked()
                    {
                        self.seed_mode = !self.seed_mode;
                        if self.seed_mode {
                            ui.memory_mut(|m| m.request_focus(seed_id));
                        }
                    }

                    if self.seed_mode {
                        let mut textin = egui::TextEdit::singleline(&mut self.seed_input)
                            .id(seed_id)
                            .hint_text("start from seed")
                            .desired_width(150.0);
                        if self.seed_errored {
                            textin = textin.text_color(egui::Color32::LIGHT_RED);
                        }
                        let resp = ui.add(textin);
                        if resp.changed() {
                            self.seed_errored = false;
                        }

                        let has_seed = self.seed_input.trim().len() > 16;

                        if (ui.add_enabled(has_seed, egui::Button::new("Go")).clicked()
                            || resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                            && has_seed
                        {
                            if self.start_from_repr(ui, self.seed_input.clone()).is_ok() {
                                self.seed_errored = false;
                                self.seed_mode = false;
                                self.seed_input.clear();
                            } else {
                                self.seed_errored = true;
                            }
                        }
                    }
                });
            });

            // --- Status line ------------------------------------------------
            ui.horizontal_wrapped(|ui| {
                ui.label(format!(
                    "Mines: {}/{}",
                    self.mines_remaining(),
                    self.difficulty.params().2
                ));
                ui.separator();

                if let Some(start_time) = self.timer_start {
                    let total_secs = (ui.time() - start_time) as u64;
                    let s = (total_secs % 60) as u8;
                    let m = ((total_secs / 60) % 60) as u8;
                    let h = (total_secs / 3600) as u8;
                    self.elapsed_time = format!("{:02}:{:02}:{:02}", h, m, s);
                    ui.request_repaint_after(std::time::Duration::from_secs(1));
                }
                ui.label(format!("Time: {}", self.elapsed_time));
                if self.status != config::GameStatus::Ready {
                    ui.separator();
                    ui.label(self.to_repr());
                }
                ui.separator();
                ui.colored_label(
                    match self.status {
                        config::GameStatus::Lost => ui.visuals().error_fg_color,
                        config::GameStatus::Won => Color32::DARK_GREEN,
                        _ => ui.visuals().text_color(),
                    },
                    self.status.label(),
                );
            });

            ui.separator();

            // --- The grid ---------------------------------------------------
            let scroll_style = &mut ui.style_mut().spacing.scroll;
            scroll_style.floating = false;
            scroll_style.bar_width = 2.0;

            egui::ScrollArea::both().show(ui, |ui| {
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
                        Rect::from_min_size(top_left, Vec2::splat(config::CELL_SIZE)).shrink(0.6);

                    // Each cell gets its own interaction response (needs a unique id).
                    let response =
                        ui.interact(cell_rect, ui.scope_id().with(("cell", idx)), Sense::click());

                    // Before the first click there is no board, so everything is hidden.
                    let state = match &self.board {
                        Some(board) => board.state(idx).unwrap(),
                        None => CellState::Hidden,
                    };

                    let is_incorrectly_marked = self.is_cell_incorrectly_marked(idx);

                    paint_cell(
                        ui,
                        cell_rect,
                        state,
                        is_incorrectly_marked,
                        response.hovered(),
                    );

                    if response.clicked() {
                        self.on_cell_click(ui, idx);
                    } else if response.secondary_clicked() {
                        self.on_cell_flagged(idx);
                    }
                }
            });

            egui::warn_if_debug_build(ui);
        });
    }

    /// Called by the framework to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }
}
