use minesweeper_core::{Board, BoardCellOpenResult, CellContent, CellState};

use crate::config::{Difficulty, GameStatus};

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct App {
    pub difficulty: Difficulty,

    /// `None` until the first click, because `Board::new` needs the start
    /// index in order to guarantee a safe first move.
    #[serde(skip)]
    pub board: Option<Board>,

    #[serde(skip)]
    pub status: GameStatus,
}

impl App {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Default::default()
        }
    }

    /// Throw away the current game and go back to the "waiting for first click" state.
    pub fn reset(&mut self) {
        self.board = None;
        self.status = GameStatus::Ready;
    }

    /// Left click on a cell.
    pub fn on_cell_click(&mut self, idx: usize) {
        if matches!(self.status, GameStatus::Won | GameStatus::Lost) {
            return;
        }

        let Some(board) = &mut self.board else {
            // First click: generate the board around the clicked cell.
            let (w, h, mines) = self.difficulty.params();
            let Ok(board) = Board::new(w, h, mines, idx) else {
                return;
            };
            self.board = Some(board);
            self.status = GameStatus::Playing;
            return;
        };

        let open_res = match board.state(idx).unwrap() {
            CellState::Flagged => Ok(BoardCellOpenResult::Noop),
            CellState::Revealed(_) => board.chord_open_cell(idx),
            CellState::Hidden => board.open_cell(idx),
        };

        if let Ok(result) = open_res {
            if result == BoardCellOpenResult::MineExploded {
                self.status = GameStatus::Lost;
            } else if result == BoardCellOpenResult::AllMinesSweeped {
                self.status = GameStatus::Won;
            }
        }
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
            .neighbours_of(idx)
            .filter(|&nidx| board.state(nidx).unwrap() == CellState::Flagged)
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
