use rand::prelude::{SeedableRng, SliceRandom, StdRng};

use crate::{
    board::*,
    solver::{self, SolverResult},
    utils,
};

/// A strategy for producing a [`Board`].
pub trait BoardGenerator {
    fn generate_from_seed(&mut self, params: &BoardParams, seed: u64) -> Result<Board, BoardError>;

    fn generate(&mut self, params: &BoardParams) -> Result<Board, BoardError> {
        self.generate_from_seed(params, rand::random())
    }
}

/// Random mine placement that keeps `start_idx` safe. Might or might not be solvable.
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
    fn generate_from_seed(
        &mut self,
        params: &BoardParams,
        mut seed: u64,
    ) -> Result<Board, BoardError> {
        loop {
            let mut board = Board::from_seed(params, seed)?;

            if solver::solve_board(&mut board) == SolverResult::Solved
                || self.perturb_and_solve_board(&mut board) == SolverResult::Solved
            {
                board.reset_state();
                // assert_eq!(solver::solve_board(&mut board), SolverResult::Solved);
                // board.reset_state();
                return Ok(board);
            }

            println!("Giving up on board and starting over");
            // make a deterministic change to the seed so that we can always
            // reproduce the same state even on an unsolvable seed input
            seed = seed.wrapping_add(1);
        }
    }
}

impl SolvableBoardGenerator {
    /// Move things around on the unsolved parts of the board and try
    /// to get it to a solvable state.
    fn perturb_and_solve_board(&self, board: &mut Board) -> SolverResult {
        let mut start_region = board.neighbours_of(board.start_idx, 1);
        let untouchable_cells: [usize; 9] =
            std::array::from_fn(|_| start_region.next().unwrap_or(usize::MAX));
        let mut rng = StdRng::seed_from_u64(board.seed);

        'perturb: loop {
            let mut frontier_candidates = Vec::<usize>::new();
            let mut interior_candidates = Vec::<usize>::new();
            // let mut known_candidates = Vec::<usize>::new();

            for i in (0..board.size()).filter(|i| !untouchable_cells.contains(i)) {
                let ncounts = utils::NeighbourInfo::for_cell(board, i);
                if board[i] == CellState::Hidden {
                    if ncounts.revealed_count() > 0 {
                        frontier_candidates.push(i);
                    } else {
                        interior_candidates.push(i);
                    }
                } else {
                    // known_candidates.push(i);
                }
            }
            frontier_candidates.shuffle(&mut rng);
            interior_candidates.shuffle(&mut rng);
            // known_candidates.shuffle(&mut rng);

            if frontier_candidates.is_empty() {
                // TODO: fix this. this happens when we have an enclosed region
                //  ~ X X ~
                //  ~ ~ ~ ~
                // For now, just skip the board
                println!("Board has enclosed region. Ignoring for now.");
                return SolverResult::Unsolvable;
            }
            // TODO: Handle moving in mines from unknown region. Will probably
            // solve for the above too

            // Find the smallest set of unknown neighbours around one constraint
            // in the frontier and transform the entire thing to a mine OR not mines.
            // That unblocks one deduction and hopefully others surrounding it.
            let (perturb_idx, _) = frontier_candidates
                .iter()
                .flat_map(|&i| board.neighbours_of(i, 1))
                .filter_map(|i| match board[i] {
                    CellState::Revealed(CellContent::Number(_)) => {
                        Some((i, utils::NeighbourInfo::for_cell(board, i).hidden_count()))
                    }
                    _ => None,
                })
                .min_by_key(|&(_, req_mines)| req_mines)
                .unwrap();

            let mut empty = Vec::new();
            let mut mines = Vec::new();
            for i in utils::NeighbourInfo::for_cell(board, perturb_idx).hidden(board) {
                match board.content(i) {
                    CellContent::Empty | CellContent::Number(_) => empty.push(i),
                    CellContent::Mine => mines.push(i),
                }
            }

            // Now we just walk the interior and then the known (in worst case) to
            // see if we can either reach nempty or nmines
            let mut found_a_move = false;
            let mut mine_donors = Vec::new();
            let mut mine_receivers = Vec::new();
            for &i in interior_candidates.iter() {
                match board.content(i) {
                    CellContent::Mine => mine_donors.push(i),
                    CellContent::Empty | CellContent::Number(_) => mine_receivers.push(i),
                }

                if mine_donors.len() >= empty.len() || mine_receivers.len() >= mines.len() {
                    found_a_move = true;
                    break;
                }
            }
            if found_a_move {
                if mine_donors.len() >= empty.len() {
                    for (&s, &d) in mine_donors.iter().zip(empty.iter()) {
                        board.move_mine(s, d);
                        board.flag_cell(d).unwrap();
                    }
                } else {
                    for (&s, &d) in mines.iter().zip(mine_receivers.iter()) {
                        board.move_mine(s, d);
                    }
                    board.open_cell(perturb_idx).unwrap();
                };
                if solver::solve_board(board) == SolverResult::Solved {
                    return SolverResult::Solved;
                }
            } else {
                break 'perturb;
            }
        }
        SolverResult::Unsolvable
    }
}
