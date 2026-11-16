use minesweeper_core::*;

use crate::config::{BoardType, Difficulty, GameStatus};

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct App {
    pub board_type: BoardType,
    pub difficulty: Difficulty,

    #[serde(skip)]
    pub icon: Option<egui::TextureHandle>,

    // seed input
    #[serde(skip)]
    pub seed_mode: bool,
    #[serde(skip)]
    pub seed_input: String,
    #[serde(skip)]
    pub seed_errored: bool,

    // timer
    #[serde(skip)]
    pub timer_start: Option<f64>,
    #[serde(skip)]
    pub elapsed_time: String,

    /// `None` until the first click, because `Board::new` needs the start
    /// index in order to guarantee a safe first move.
    #[serde(skip)]
    pub board: Option<Board>,

    #[serde(skip)]
    pub status: GameStatus,
}

impl Default for App {
    fn default() -> Self {
        Self {
            icon: None,
            board_type: BoardType::Solvable,
            difficulty: Difficulty::Intermediate,
            seed_mode: false,
            seed_input: Default::default(),
            seed_errored: false,
            timer_start: None,
            elapsed_time: "00:00:00".to_owned(),
            board: None,
            status: GameStatus::Ready,
        }
    }
}

impl App {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        let mut state: Self = if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Default::default()
        };

        if let Ok(icon_data) =
            eframe::icon_data::from_png_bytes(&include_bytes!("../assets/icon.png")[..])
        {
            state.icon = Some(cc.egui_ctx.load_texture(
                "app_icon",
                egui::ColorImage::from_rgba_unmultiplied(
                    [icon_data.width as usize, icon_data.height as usize],
                    &icon_data.rgba,
                ),
                egui::TextureOptions::LINEAR,
            ));
        }
        state
    }

    /// Throw away the current game and go back to the "waiting for first click" state.
    pub fn reset(&mut self) {
        self.board = None;
        self.status = GameStatus::Ready;
        self.timer_start = None;
        self.elapsed_time = "00:00:00".to_owned();
    }

    pub fn to_repr(&self) -> String {
        let d = match self.difficulty {
            Difficulty::Beginner => "b",
            Difficulty::Intermediate => "i",
            Difficulty::Expert => "e",
            Difficulty::Master => "m",
        };
        let b = match self.board_type {
            BoardType::Simple => "0",
            BoardType::Solvable => "1",
        };
        format!(
            "Seed: {:x}x{:x}{}{}",
            self.get_seed(),
            self.get_start(),
            d,
            b
        )
    }

    pub fn parse_repr(&self, seed_str: &str) -> Option<(u64, usize, Difficulty, BoardType)> {
        let (rng_part, config_part) = seed_str.trim().split_once('x')?;

        let l = config_part.len();
        if l < 3 {
            return None;
        }
        let idx_part = &config_part[..l - 2];
        let d = &config_part[l - 2..l - 1];
        let b = &config_part[l - 1..];

        let rng_seed = u64::from_str_radix(rng_part, 16).ok()?;
        let start_idx = usize::from_str_radix(idx_part, 16).ok()?;

        let difficulty = match d {
            "b" => Difficulty::Beginner,
            "i" => Difficulty::Intermediate,
            "e" => Difficulty::Expert,
            "m" => Difficulty::Master,
            _ => return None,
        };

        let board_type = match b {
            "0" => BoardType::Simple,
            "1" => BoardType::Solvable,
            _ => return None,
        };

        Some((rng_seed, start_idx, difficulty, board_type))
    }

    pub fn start_from_repr(&mut self, ctx: &egui::Ui, seed_str: String) -> Result<(), BoardError> {
        let (seed, start_idx, difficulty, board_type) =
            self.parse_repr(&seed_str).ok_or(BoardError::InvalidInput)?;

        self.reset();
        self.difficulty = difficulty;
        self.board_type = board_type;
        let (w, h, mines) = self.difficulty.params();
        let board_gen: &'static dyn BoardGenerator = match self.board_type {
            BoardType::Simple => &SimpleBoardGenerator,
            BoardType::Solvable => &SolvableBoardGenerator,
        };
        let board = board_gen.generate_from_seed(
            &BoardParams {
                width: w,
                height: h,
                mine_count: mines,
                start_idx,
            },
            seed,
        )?;
        self.board = Some(board);
        self.update_game_status(
            ctx,
            if self.board.as_ref().unwrap().is_won() {
                GameStatus::Won
            } else {
                GameStatus::Playing
            },
        );

        Ok(())
    }

    pub fn update_game_status(&mut self, ctx: &egui::Ui, status: GameStatus) {
        if self.status == status {
            return;
        }
        self.status = status;
        if status == GameStatus::Playing {
            self.timer_start = Some(ctx.time())
        } else {
            self.timer_start = None;
        }
    }

    /// Left click on a cell.
    pub fn on_cell_click(&mut self, ctx: &egui::Ui, idx: usize) {
        if matches!(self.status, GameStatus::Won | GameStatus::Lost) {
            return;
        }

        let open_res = if let Some(board) = &mut self.board {
            match board.state(idx).unwrap() {
                CellState::Flagged => BoardCellOpenResult::Noop,
                CellState::Revealed(_) => board
                    .chord_open_cell(idx)
                    .unwrap_or(BoardCellOpenResult::Noop),
                CellState::Hidden => board.open_cell(idx).unwrap_or(BoardCellOpenResult::Noop),
            }
        } else {
            // First click: generate the board around the clicked cell.
            let (w, h, mines) = self.difficulty.params();
            let board_gen: &'static dyn BoardGenerator = match self.board_type {
                BoardType::Simple => &SimpleBoardGenerator,
                BoardType::Solvable => &SolvableBoardGenerator,
            };
            let Ok(board) = board_gen.generate(&BoardParams {
                width: w,
                height: h,
                mine_count: mines,
                start_idx: idx,
            }) else {
                return;
            };
            self.board = Some(board);
            if self.board.as_ref().unwrap().is_won() {
                BoardCellOpenResult::AllMinesSweeped
            } else {
                BoardCellOpenResult::Noop
            }
        };
        self.update_game_status(
            ctx,
            match open_res {
                BoardCellOpenResult::AllMinesSweeped => GameStatus::Won,
                BoardCellOpenResult::MineExploded => GameStatus::Lost,
                _ => GameStatus::Playing,
            },
        );
    }

    /// Right click on a cell.
    pub fn on_cell_flagged(&mut self, idx: usize) {
        if self.status != GameStatus::Playing {
            return; // can't flag before the board exists, or after the game ended
        }
        let Some(board) = &mut self.board else {
            return;
        };

        let _ = match board.state(idx).unwrap() {
            CellState::Flagged => board.unflag_cell(idx),
            CellState::Hidden => board.flag_cell(idx),
            _ => Ok(()),
        };
    }

    /// Mines minus flags placed (can go negative if the player over-flags).
    pub fn mines_remaining(&self) -> isize {
        self.board.as_ref().map_or(0, |b| b.mines_remaining())
    }

    /// whether a cell has more mines marked around it than the actual num
    pub fn is_cell_incorrectly_marked(&self, idx: usize) -> bool {
        let Some(board) = &self.board else {
            return false;
        };

        let CellState::Revealed(CellContent::Number(cell_num)) = board.state(idx).unwrap() else {
            return false;
        };

        board
            .neighbours_of(idx, 1)
            .filter(|&nidx| board[nidx] == CellState::Flagged)
            .count() as u8
            > cell_num
    }

    pub fn get_seed(&self) -> u64 {
        self.board.as_ref().map_or(0, |b| b.seed)
    }

    pub fn get_start(&self) -> usize {
        self.board.as_ref().map_or(0, |b| b.start_idx)
    }
}
