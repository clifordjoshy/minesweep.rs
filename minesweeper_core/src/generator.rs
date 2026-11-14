use rand::prelude::{IndexedRandom, IteratorRandom, SeedableRng, SmallRng};

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
        // let mut start_region = board.neighbours_of(board.start_idx, 1);
        // let untouchable_cells: [usize; 9] =
        //     std::array::from_fn(|_| start_region.next().unwrap_or(usize::MAX));

        let mut rng = SmallRng::seed_from_u64(board.seed);

        'perturb: loop {
            // Find the smallest set of unknown neighbours around one constraint
            // in the frontier and transform the entire thing to a mine OR not mines.
            // That unblocks one deduction and hopefully others surrounding it.
            let Some(perturb_idx) = (0..board.size())
                .filter_map(|i| {
                    if let CellState::Revealed(CellContent::Number(_)) = board[i] {
                        let hcount = utils::NeighbourInfo::for_cell(board, i).hidden_count();
                        (hcount > 0).then_some((i, hcount))
                    } else {
                        None
                    }
                })
                .fold(
                    (8u8, Vec::<usize>::new()),
                    |(cur_min, mut idxs), (i, req_mines)| {
                        if req_mines > cur_min {
                            (cur_min, idxs)
                        } else if req_mines == cur_min {
                            idxs.push(i);
                            (req_mines, idxs)
                        } else {
                            idxs.clear();
                            idxs.push(i);
                            (req_mines, idxs)
                        }
                    },
                )
                .1
                .choose(&mut rng)
                .copied()
            else {
                // TODO: fix this. this happens when we have an enclosed region
                // (due to a lack of frontier)
                //  ~ X X ~
                //  ~ ~ ~ ~
                // For now, just skip the board
                println!("Board has enclosed region. Ignoring for now.");
                return SolverResult::Unsolvable;
            };

            // TODO: Handle moving in mines from unknown region. Will probably
            // solve for the above too. This pretty much only happens for the
            // last couple of cells or the above enclosed case.

            let (mines, empty): (Vec<_>, Vec<_>) =
                utils::NeighbourInfo::for_cell(board, perturb_idx)
                    .hidden(board)
                    .partition(|&i| board.content(i) == CellContent::Mine);

            // Now we just walk the interior and then the known (in worst case) to
            // see if we can either reach nempty or nmines
            let mut found_a_move = false;
            let mut mine_donors = Vec::new();
            let mut mine_receivers = Vec::new();

            let interior_candidates = (0..board.size())
                .filter(|&i| {
                    board[i] == CellState::Hidden
                        && board
                            .neighbours_of(i, 1)
                            .all(|n| board[n] == CellState::Hidden)
                })
                .sample(&mut rng, mines.len() + empty.len());
            // random subset sample since we only ever read up to 8 before one of
            // the conditions is fulfilled.

            for i in interior_candidates {
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
                    }
                    for d in empty {
                        board.flag_cell(d).unwrap();
                    }
                } else {
                    for (&s, &d) in mines.iter().zip(mine_receivers.iter()) {
                        board.move_mine(s, d);
                        board.open_cell(s).unwrap();
                    }
                    for s in mines {
                        board.open_cell(s).unwrap();
                    }
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
