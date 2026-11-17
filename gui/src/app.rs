use minesweeper_core::*;

use crate::config::{self, BoardType, Difficulty, GameStatus};

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct App {
    pub board_type: BoardType,
    pub difficulty: Difficulty,

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
    pub board: Option<Board>,

    #[serde(skip)]
    pub status: GameStatus,
}

impl Default for App {
    fn default() -> Self {
        Self {
            board_type: BoardType::Solvable,
            difficulty: config::DEFAULT_DIFFICULTY,
            seed_mode: false,
            seed_input: Default::default(),
            seed_errored: false,
            timer_start: None,
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
        cc.storage
            .and_then(|storage| eframe::get_value(storage, eframe::APP_KEY))
            .unwrap_or_else(|| {
                cc.egui_ctx.set_theme(config::DEFAULT_THEME);
                Default::default()
            })
    }

    /// Throw away the current game and go back to the "waiting for first click" state.
    pub fn reset(&mut self) {
        self.board = None;
        self.status = GameStatus::Ready;
        self.timer_start = None;
    }

    pub fn start_from_repr(&mut self, ui: &egui::Ui, seed_str: String) -> Result<(), BoardError> {
        let game_seed = seed_str
            .parse::<config::GameSeed>()
            .map_err(|_| BoardError::InvalidInput)?;
        let (w, h, mines) = game_seed.difficulty.params();

        let board = self.board_type.generator().generate_from_seed(
            &BoardParams {
                width: w,
                height: h,
                mine_count: mines,
                start_idx: game_seed.start_idx,
            },
            game_seed.rng_seed,
        )?;
        self.reset();
        self.difficulty = game_seed.difficulty;
        self.fit_window_to_board(ui);
        self.board_type = game_seed.board_type;
        let is_won = board.is_won();
        self.board = Some(board);

        self.update_game_status(
            ui,
            if is_won {
                GameStatus::Won
            } else {
                GameStatus::Playing
            },
        );

        Ok(())
    }

    pub fn update_game_status(&mut self, ui: &egui::Ui, status: GameStatus) {
        if self.status == status {
            return;
        }
        self.status = status;
        if status == GameStatus::Playing {
            self.timer_start = Some(ui.time())
        } else {
            self.timer_start = None;
        }
    }

    /// Left click on a cell.
    pub fn on_cell_click(&mut self, ui: &egui::Ui, idx: usize) {
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
            let Ok(board) = self.board_type.generator().generate(&BoardParams {
                width: w,
                height: h,
                mine_count: mines,
                start_idx: idx,
            }) else {
                return;
            };
            let is_won_already = board.is_won();
            self.board = Some(board);
            if is_won_already {
                BoardCellOpenResult::AllMinesSweeped
            } else {
                BoardCellOpenResult::Noop
            }
        };
        self.update_game_status(
            ui,
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
