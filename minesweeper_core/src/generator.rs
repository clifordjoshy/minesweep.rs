use crate::{board::*, solver::solve_board};

/// A strategy for producing a [`Board`].
pub trait BoardGenerator {
    fn generate_from_seed(&mut self, params: &BoardParams, seed: u64) -> Result<Board, BoardError>;

    fn generate(&mut self, params: &BoardParams) -> Result<Board, BoardError> {
        self.generate_from_seed(params, rand::random())
    }
}

/// Random mine placement that keeps `start_idx` safe. Might or might not be solvable
/// Similar to the original Minesweeper implementation
pub struct SimpleBoardGenerator;
impl BoardGenerator for SimpleBoardGenerator {
    fn generate_from_seed(&mut self, params: &BoardParams, seed: u64) -> Result<Board, BoardError> {
        Board::from_seed(params, seed)
    }
}

/// Generator that guarantees that the returned [`Board`] is solvable using
/// logical deductions
pub struct SolvableBoardGenerator;
impl BoardGenerator for SolvableBoardGenerator {
    fn generate_from_seed(&mut self, params: &BoardParams, seed: u64) -> Result<Board, BoardError> {
        let mut board = Board::from_seed(params, seed)?;
        // todo
        solve_board(&mut board);
        Ok(board)
    }
}
