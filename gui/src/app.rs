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
    pub seed_error: Option<String>,

    // timer
    #[serde(skip)]
    pub timer_start: Option<f64>,
    #[serde(skip)]
    pub elapsed_seconds: u64,

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
            seed_error: None,
            timer_start: None,
            elapsed_seconds: 0,
            board: None,
            status: GameStatus::Waiting,
        }
    }
}

impl App {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.
        egui_extras::install_image_loaders(&cc.egui_ctx);

        // Load previous app state (if any).
        cc.storage
            .and_then(|storage| eframe::get_value(storage, eframe::APP_KEY))
            .unwrap_or_else(|| {
                cc.egui_ctx
                    .set_theme(cc.egui_ctx.system_theme().unwrap_or(config::DEFAULT_THEME));
                Default::default()
            })
    }

    /// Throw away the current game and go back to the "waiting for first click" state.
    pub fn reset(&mut self) {
        self.board = None;
        self.status = GameStatus::Waiting;
        self.timer_start = None;
        self.elapsed_seconds = 0;
    }

    pub fn start_from_game_seed(
        &mut self,
        ui: &egui::Ui,
        game_seed: &GameSeed,
    ) -> Result<(), BoardError> {
        let (w, h, mines) = game_seed.difficulty.params();

        let board = game_seed.board_type.generator().generate_from_seed(
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
        self.fit_window_to_board(ui.ctx());
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

    pub fn get_game_seed(&self) -> Option<GameSeed> {
        let Some(board) = &self.board else {
            return None;
        };

        Some(GameSeed {
            difficulty: self.difficulty,
            board_type: self.board_type,
            rng_seed: board.seed,
            start_idx: board.start_idx,
        })
    }
}

/// everything required to represent a game start state
#[derive(Debug)]
pub struct GameSeed {
    pub difficulty: Difficulty,
    pub board_type: BoardType,
    pub rng_seed: u64,
    pub start_idx: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseGameSeedError {
    NotAscii,
    MissingSeparator,
    InvalidLength,
    InvalidHex,
    InvalidDifficulty,
    InvalidBoardType,
}

impl std::fmt::Display for GameSeed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let d = match self.difficulty {
            Difficulty::Beginner => 'b',
            Difficulty::Intermediate => 'i',
            Difficulty::Expert => 'e',
            Difficulty::Master => 'm',
        };
        let b = match self.board_type {
            BoardType::Simple => '0',
            BoardType::Solvable => '1',
        };
        write!(f, "{:x}x{:x}{}{}", self.rng_seed, self.start_idx, d, b)
    }
}

impl std::str::FromStr for GameSeed {
    type Err = ParseGameSeedError;

    fn from_str(seed_str: &str) -> Result<Self, Self::Err> {
        let trimmed = seed_str.trim();

        if !trimmed.is_ascii() {
            return Err(ParseGameSeedError::NotAscii);
        }

        let (rng_part, config_part) = trimmed
            .split_once('x')
            .ok_or(ParseGameSeedError::MissingSeparator)?;

        let l = config_part.len();
        if l < 3 {
            return Err(ParseGameSeedError::InvalidLength);
        }

        let idx_part = &config_part[..l - 2];
        let d = &config_part[l - 2..l - 1];
        let b = &config_part[l - 1..];

        let rng_seed =
            u64::from_str_radix(rng_part, 16).map_err(|_| ParseGameSeedError::InvalidHex)?;
        let start_idx =
            usize::from_str_radix(idx_part, 16).map_err(|_| ParseGameSeedError::InvalidHex)?;

        let difficulty = match d {
            "b" => Difficulty::Beginner,
            "i" => Difficulty::Intermediate,
            "e" => Difficulty::Expert,
            "m" => Difficulty::Master,
            _ => return Err(ParseGameSeedError::InvalidDifficulty),
        };

        let board_type = match b {
            "0" => BoardType::Simple,
            "1" => BoardType::Solvable,
            _ => return Err(ParseGameSeedError::InvalidBoardType),
        };

        Ok(Self {
            rng_seed,
            start_idx,
            difficulty,
            board_type,
        })
    }
}
