use crate::app;
use crate::config;
use egui::emath::GuiRounding;
use egui::{Align2, Color32, FontId, Painter, Rect, Sense, Shape, Stroke, Vec2, vec2};
use minesweeper_core::{CellContent, CellState};

enum Glyph {
    Text(&'static str, Color32),
    Flag,
    Mine,
}

fn paint_flag(painter: &Painter, rect: Rect, colors: &config::Colors) {
    let p = |fx: f32, fy: f32| rect.min + vec2(fx * rect.width(), fy * rect.height());
    let pole_w = rect.width() * 0.07;

    painter.line_segment(
        [p(0.52, 0.20), p(0.52, 0.78)],
        Stroke::new(pole_w, colors.flag_pole),
    );
    painter.rect_filled(
        Rect::from_min_max(p(0.35, 0.78), p(0.68, 0.86)),
        1.0,
        colors.flag_pole,
    );
    painter.add(Shape::convex_polygon(
        vec![p(0.56, 0.16), p(0.56, 0.50), p(0.18, 0.35), p(0.18, 0.31)],
        colors.flag_cloth,
        Stroke::NONE,
    ));
}

fn paint_mine(painter: &Painter, rect: Rect, colors: &config::Colors) {
    let c = rect.center();
    let w = rect.width();
    let color = colors.mine_glyph;

    let body_r = w * 0.20;
    let spike_len = w * 0.26;
    let stroke = Stroke::new(w * 0.07, color);

    for i in 0..4 {
        let d = Vec2::angled(i as f32 * std::f32::consts::FRAC_PI_4) * spike_len;
        painter.line_segment([c - d, c + d], stroke);
    }

    painter.circle_filled(c, body_r, color);
}

fn paint_cell(
    ui: &egui::Ui,
    rect: Rect,
    colors: &config::Colors,
    state: CellState,
    incorrectly_marked: bool,
    highlighted: bool,
) {
    let painter = ui.painter();

    let (bg, glyph): (Color32, Option<Glyph>) = match state {
        CellState::Hidden => {
            let bg = if highlighted {
                colors.hidden_hover
            } else {
                colors.hidden
            };
            (bg, None)
        }
        CellState::Flagged => (colors.hidden, Some(Glyph::Flag)),
        CellState::Revealed(content) => match content {
            CellContent::Empty => (colors.revealed, None),
            CellContent::Number(n) => {
                let bg = if incorrectly_marked {
                    colors.incorrect
                } else {
                    colors.revealed
                };
                (
                    bg,
                    Some(Glyph::Text(
                        config::DIGITS_TEXT[n as usize],
                        colors.number(n),
                    )),
                )
            }
            CellContent::Mine => (colors.mine_bg, Some(Glyph::Mine)),
        },
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
        Some(Glyph::Flag) => paint_flag(painter, rect, colors),
        Some(Glyph::Mine) => paint_mine(painter, rect, colors),
        None => {}
    }
}

impl eframe::App for app::App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            let colors = config::Colors::for_theme(ui.ctx().theme());

            ui.spacing_mut().item_spacing.y = 5.;

            ui.horizontal(|ui| {
                ui.add(
                    egui::Image::new(egui::include_image!("../assets/icon.svg"))
                        .max_height(ui.text_style_height(&egui::TextStyle::Heading)),
                );
                ui.heading(config::APP_NAME);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let is_dark = ui.ctx().theme() == egui::Theme::Dark;

                    let (icon, tooltip) = if is_dark {
                        ("☀️", "Switch to Light Mode")
                    } else {
                        ("🌙", "Switch to Dark Mode")
                    };

                    if ui.button(icon).on_hover_text(tooltip).clicked() {
                        ui.ctx().set_theme(if is_dark {
                            egui::Theme::Light
                        } else {
                            egui::Theme::Dark
                        });
                    }
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
                    self.fit_window_to_board(ui.ctx());
                }
            });

            // --- Controls: Board Type  + new game button -------------
            ui.horizontal_wrapped(|ui| {
                let mut changed = false;
                for b in config::BoardType::ALL {
                    if ui
                        .selectable_value(&mut self.board_type, b, b.label())
                        .on_hover_text(b.description())
                        .changed()
                    {
                        changed = true;
                    }
                }
                if changed {
                    self.reset();
                }

                ui.separator();
                ui.add_space(ui.spacing().item_spacing.x);
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

                    let arrow_btn =
                        ui.add(egui::Button::new(" ").corner_radius(egui::CornerRadius {
                            nw: 0,
                            ne: 4,
                            sw: 0,
                            se: 4,
                        }));

                    ui.painter().add(egui::Shape::convex_polygon(
                        {
                            let c = arrow_btn.rect.center();
                            let w = arrow_btn.rect.width() * 0.4;
                            let h = arrow_btn.rect.height() * 0.4;
                            let inv = if self.seed_mode { -1 } else { 1 } as f32;

                            vec![
                                egui::pos2(c.x + w * 0.5 * inv, c.y),
                                egui::pos2(c.x - w * 0.5 * inv, c.y - h * 0.5),
                                egui::pos2(c.x - w * 0.5 * inv, c.y + h * 0.5),
                            ]
                        },
                        ui.style().interact(&arrow_btn).text_color(),
                        egui::Stroke::NONE,
                    ));

                    if arrow_btn.clicked() {
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
                        if self.seed_error.is_some() {
                            textin = textin.text_color(colors.lost);
                        }
                        let mut resp = ui.add(textin);

                        if let Some(err_label) = self.seed_error.as_ref() {
                            resp = resp.on_hover_ui(|ui| {
                                ui.label(err_label);
                            });
                        }

                        if resp.changed() {
                            self.seed_error = None;
                        }

                        if ui
                            .add_enabled(
                                !self.seed_input.trim().is_empty(),
                                egui::Button::new("Go"),
                            )
                            .clicked()
                            || resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
                        {
                            match self
                                .seed_input
                                .parse::<app::GameSeed>()
                                .map_err(|e| format!("{:?}", e))
                                .and_then(|game_seed| {
                                    self.start_from_game_seed(ui, &game_seed)
                                        .map_err(|e| format!("{:?}", e))
                                }) {
                                Ok(_) => {
                                    self.seed_error = None;
                                    self.seed_mode = false;
                                    self.seed_input.clear();
                                }
                                Err(err_msg) => {
                                    self.seed_error = Some(err_msg);
                                }
                            }
                        }
                    }
                });
            });

            // --- Status line ------------------------------------------------
            ui.horizontal_wrapped(|ui| {
                if self.status != config::GameStatus::Waiting {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.;
                        ui.label("Mines: ");
                        ui.label(
                            egui::RichText::new(format!(
                                "{}/{}",
                                self.mines_remaining(),
                                self.difficulty.params().2
                            ))
                            .monospace(),
                        );
                    });

                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.;
                        ui.label("Time: ");

                        if let Some(start_time) = self.timer_start {
                            self.elapsed_seconds = (ui.time() - start_time) as u64;
                            ui.request_repaint_after(std::time::Duration::from_secs(1));
                        }
                        let s = (self.elapsed_seconds % 60) as u8;
                        let m = ((self.elapsed_seconds / 60) % 60) as u8;
                        let h = (self.elapsed_seconds / 3600) as u32;
                        ui.label(
                            egui::RichText::new(format!("{:02}:{:02}:{:02}", h, m, s)).monospace(),
                        );
                    });

                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.;
                        ui.label("Seed: ");
                        ui.label(
                            egui::RichText::new(self.get_game_seed().unwrap().to_string())
                                .monospace(),
                        );
                    });
                    ui.separator();
                }

                ui.colored_label(
                    match self.status {
                        config::GameStatus::Lost => colors.lost,
                        config::GameStatus::Won => colors.won,
                        _ => ui.visuals().text_color(),
                    },
                    self.status.label(),
                );
            });

            ui.separator();

            // --- The grid ---------------------------------------------------
            let scroll_style = &mut ui.style_mut().spacing.scroll;
            scroll_style.floating = false;
            scroll_style.bar_width = 4.0;

            let (w, h, _) = self.difficulty.params();

            // Reserve one rectangle for the whole board, then carve cells out of it.
            let board_size = vec2(w as f32 * config::CELL_SIZE, h as f32 * config::CELL_SIZE);
            let available = ui.available_size();

            let pad_x = ((available.x - board_size.x) * 0.5).max(0.0);
            let pad_y =
                (((available.y - board_size.y) * 0.5) - ui.spacing().item_spacing.y).max(0.0);

            egui::ScrollArea::both().show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.;
                ui.spacing_mut().item_spacing.x = 0.;
                ui.add_space(pad_y);
                ui.horizontal(|ui| {
                    ui.add_space(pad_x);
                    let (board_rect, _) = ui.allocate_exact_size(board_size, Sense::hover());
                    let origin = board_rect.min;

                    let ppp = ui.pixels_per_point();
                    let gap = 1.2 / ppp;

                    for idx in 0..w * h {
                        let (x, y) = (idx % w, idx / w);

                        // Snap the outer grid boundaries of each cell to exact physical pixels
                        let min_x = (origin.x + x as f32 * config::CELL_SIZE).round_to_pixels(ppp);
                        let max_x =
                            (origin.x + (x + 1) as f32 * config::CELL_SIZE).round_to_pixels(ppp);
                        let min_y = (origin.y + y as f32 * config::CELL_SIZE).round_to_pixels(ppp);
                        let max_y =
                            (origin.y + (y + 1) as f32 * config::CELL_SIZE).round_to_pixels(ppp);

                        let cell_rect = Rect::from_min_max(
                            egui::pos2(min_x + gap * 0.5, min_y + gap * 0.5),
                            egui::pos2(max_x - gap * 0.5, max_y - gap * 0.5),
                        );

                        // Each cell gets its own interaction response (needs a unique id).
                        let response = ui.interact(
                            cell_rect,
                            ui.scope_id().with(("cell", idx)),
                            Sense::click(),
                        );

                        // Before the first click there is no board, so everything is hidden.
                        let state = match &self.board {
                            Some(board) => board.state(idx).unwrap(),
                            None => CellState::Hidden,
                        };

                        let is_incorrectly_marked = self.is_cell_incorrectly_marked(idx);

                        let is_highlighted = response.hovered()
                            && matches!(
                                self.status,
                                config::GameStatus::Waiting | config::GameStatus::Playing
                            )
                            && matches!(state, CellState::Hidden | CellState::Flagged);

                        paint_cell(
                            ui,
                            cell_rect,
                            colors,
                            state,
                            is_incorrectly_marked,
                            is_highlighted,
                        );

                        if is_highlighted {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }

                        if response.clicked() {
                            self.on_cell_click(ui, idx);
                        } else if response.secondary_clicked() {
                            self.on_cell_flagged(idx);
                        }
                    }
                });
            });

            egui::warn_if_debug_build(ui);
        });
    }

    /// Called by the framework to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }
}

impl app::App {
    pub fn window_size_for_difficulty(d: config::Difficulty) -> egui::Vec2 {
        let (w, h, _) = d.params();
        egui::vec2(
            (w as f32 * config::CELL_SIZE + 24.).max(600.),
            h as f32 * config::CELL_SIZE + 160.0,
        )
    }

    pub fn fit_window_to_board(&self, ctx: &egui::Context) {
        let (maximized, fullscreen, minimized) = ctx.input(|i| {
            let vp = i.viewport();
            (vp.maximized, vp.fullscreen, vp.minimized)
        });
        if maximized == Some(true) || fullscreen == Some(true) || minimized == Some(true) {
            return;
        }

        let mut needed = Self::window_size_for_difficulty(self.difficulty);

        let current = ctx.input(|i| i.viewport().inner_rect.map(|r| r.size()));
        let monitor = ctx.input(|i| i.viewport().monitor_size);

        if let Some(cur) = current {
            if let Some(m) = monitor {
                needed = needed.min(m * 0.95); // don't exceed the screen
            }
            if needed != cur {
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(needed));
            }
        }
    }
}
