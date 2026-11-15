use std::collections::VecDeque;

use rand::prelude::{IndexedRandom, IteratorRandom, SeedableRng, SmallRng};

use crate::{
    board::*,
    solver::{self, SolverResult},
    utils,
};

/// A strategy for producing a [`Board`].
pub trait BoardGenerator {
    fn generate_from_seed(&self, params: &BoardParams, seed: u64) -> Result<Board, BoardError>;

    fn generate(&self, params: &BoardParams) -> Result<Board, BoardError> {
        self.generate_from_seed(params, rand::random())
    }
}
/// Random mine placement that keeps `start_idx` safe. Might or might not be solvable.
/// Similar to the original Minesweeper implementation
pub struct SimpleBoardGenerator;
impl BoardGenerator for SimpleBoardGenerator {
    fn generate_from_seed(&self, params: &BoardParams, seed: u64) -> Result<Board, BoardError> {
        Board::from_seed(params, seed)
    }
}

/// Generator that guarantees that the returned [`Board`] is solvable using
/// logical deductions
pub struct SolvableBoardGenerator;
impl BoardGenerator for SolvableBoardGenerator {
    fn generate_from_seed(&self, params: &BoardParams, mut seed: u64) -> Result<Board, BoardError> {
        loop {
            let mut board = Board::from_seed(params, seed)?;

            if solver::solve_board(&mut board, vec![params.start_idx]) == SolverResult::Solved
                || self.perturb_and_solve_board(&mut board) == SolverResult::Solved
            {
                board.reset_state();
                // assert_eq!(
                //     solver::solve_board(&mut board, vec![params.start_idx]),
                //     SolverResult::Solved
                // );
                // board.reset_state();
                return Ok(board);
            }

            log::debug!("Giving up on board and starting over");
            // make a deterministic change to the seed so that we can always
            // reproduce the same state even on an unsolvable seed input
            seed = seed.wrapping_add(1);
        }
    }
}

impl SolvableBoardGenerator {
    fn perturb_and_solve_board(&self, board: &mut Board) -> SolverResult {
        let mut rng = SmallRng::seed_from_u64(board.seed);

        let mut nperturbs_prev = u32::MAX;
        let mut rounds = 1;
        loop {
            let (nperturbs, is_still_guaranteed_solvable) = self.perturb(board, &mut rng);
            log::debug!(
                "Perturb round {}: Made {} perturbations. Affected previous deductions: {}",
                rounds,
                nperturbs,
                !is_still_guaranteed_solvable
            );

            if is_still_guaranteed_solvable || {
                board.reset_state();
                solver::solve_board(board, vec![board.start_idx]) == SolverResult::Solved
            } {
                return SolverResult::Solved;
            }

            // The perturbations we made messed up the board somehow. Re-run
            // and hope that the numperturbs is reducing each run.
            // If it doesn't, then we must assume that things are getting worse
            // and just return that this seed creates an unsolvable board
            if nperturbs > nperturbs_prev {
                log::debug!(
                    "Board is getting worse. Giving up. ({} > {})",
                    nperturbs,
                    nperturbs_prev
                );
                return SolverResult::Unsolvable;
            }
            nperturbs_prev = nperturbs;
            rounds += 1;
        }
    }

    /// Move things around on the unsolved/solved parts of the board and try
    /// to get it to a solvable state. This could cause the resulting board to
    /// go into an unsolvable state.
    /// Returns (num_perturbs, is_still_guaranteed_solvable)
    fn perturb(&self, board: &mut Board, rng: &mut SmallRng) -> (u32, bool) {
        let mut could_have_messed_up_existing = false;
        let mut nperturbs = 0u32;

        loop {
            // Find the smallest set of unknown neighbours around one constraint
            // in the frontier and transform the entire thing to a mine OR not mines.
            // That unblocks one deduction and hopefully others surrounding it.
            let maybe_perturb_idx = (0..board.size())
                .filter_map(|i| {
                    if let CellState::Revealed(CellContent::Number(_)) = board[i] {
                        let hcount = utils::NeighbourInfo::for_cell(board, i).hidden_count();
                        (hcount > 0).then_some((i, hcount))
                    } else {
                        None
                    }
                })
                .fold(
                    (u8::MAX, Vec::<usize>::new()),
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
                .choose(rng)
                .copied();

            let Some(perturb_idx) = maybe_perturb_idx else {
                //  We only have enclosed regions, so no frontier and no perturb index
                //  ~ X X ~
                //  ~ ~ ~ ~

                let modified_indices = self.handle_enclosed_regions(board, rng);
                nperturbs += 1;
                could_have_messed_up_existing = true;
                if solver::solve_board(board, modified_indices) == SolverResult::Solved {
                    return (nperturbs, false);
                }
                continue;
            };

            let (pmines, pempty) = utils::NeighbourInfo::for_cell(board, perturb_idx)
                .hidden(board)
                .partition::<Vec<_>, _>(|&i| board.content(i) == CellContent::Mine);

            // Now we just walk the interior and then the frontier, known(in worst case) to
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
                .sample(rng, pmines.len() + pempty.len());
            // random subset sample since we only ever read up to 8 before one of
            // the conditions is fulfilled.

            for i in interior_candidates {
                match board.content(i) {
                    CellContent::Mine => mine_donors.push(i),
                    CellContent::Empty | CellContent::Number(_) => mine_receivers.push(i),
                }

                if mine_donors.len() >= pempty.len() || mine_receivers.len() >= pmines.len() {
                    found_a_move = true;
                    break;
                }
            }

            if !found_a_move {
                let frontier_candidates = (0..board.size())
                    .filter(|&i| {
                        board[i] == CellState::Hidden
                            && board
                                .neighbours_of(i, 1)
                                .any(|n| matches!(board[n], CellState::Revealed(_)))
                            && !board.neighbours_of(perturb_idx, 1).any(|p| p == i)
                    })
                    .sample(
                        rng,
                        pmines.len() + pempty.len() - mine_donors.len() - mine_receivers.len(),
                    );

                for i in frontier_candidates {
                    match board.content(i) {
                        CellContent::Mine => mine_donors.push(i),
                        CellContent::Empty | CellContent::Number(_) => mine_receivers.push(i),
                    }

                    if mine_donors.len() >= pempty.len() || mine_receivers.len() >= pmines.len() {
                        found_a_move = true;
                        could_have_messed_up_existing = true;
                        break;
                    }
                }
            }

            if !found_a_move {
                let revealed_candidates = (0..board.size())
                    .filter(|&i| {
                        matches!(board[i], CellState::Revealed(_)) && !board.is_in_start_region(i)
                    })
                    .sample(
                        rng,
                        pmines.len() + pempty.len() - mine_donors.len() - mine_receivers.len(),
                    );

                for i in revealed_candidates {
                    match board.content(i) {
                        CellContent::Mine => mine_donors.push(i),
                        CellContent::Empty | CellContent::Number(_) => mine_receivers.push(i),
                    }

                    if mine_donors.len() >= pempty.len() || mine_receivers.len() >= pmines.len() {
                        found_a_move = true;
                        could_have_messed_up_existing = true;
                        break;
                    }
                }
            }

            assert!(found_a_move);
            nperturbs += 1;

            let modified_indices = if mine_donors.len() >= pempty.len() {
                for (&s, &d) in mine_donors.iter().zip(pempty.iter()) {
                    board.move_mine(s, d);
                }
                for &d in pempty.iter() {
                    board.flag_cell(d).unwrap();
                }
                pempty
            } else {
                for (&s, &d) in pmines.iter().zip(mine_receivers.iter()) {
                    board.move_mine(s, d);
                    board.open_cell(s).unwrap();
                }
                for &s in pmines.iter() {
                    board.open_cell(s).unwrap();
                }
                pmines
            };
            if solver::solve_board(board, modified_indices) == SolverResult::Solved {
                return (nperturbs, !could_have_messed_up_existing);
            }
        }
    }

    // We hit a standstill on the perturbs when the board has a region enclosed
    // in mines. This function will open up the enclosed regions by finding all
    // disconnected boundaries and kick a mine from each of them into the interior
    // (or the known empty region)
    // Returns the indices modified
    fn handle_enclosed_regions(&self, board: &mut Board, rng: &mut SmallRng) -> Vec<usize> {
        // --- BFS to split into disjoint boundaries
        let mut mines_to_clear = Vec::<usize>::new();

        let mut visited_cells_flags = vec![false; board.size()];
        let hidden_interior_cells = (0..board.size())
            .filter(|&i| {
                board[i] == CellState::Hidden
                    && board
                        .neighbours_of(i, 1)
                        .all(|n| board[n] == CellState::Flagged || board[n] == CellState::Hidden)
            })
            .collect::<Vec<_>>();

        for &i in hidden_interior_cells.iter() {
            if visited_cells_flags[i] {
                continue;
            }
            let mut current_boundary = Vec::<usize>::new();
            let mut visit_queue = VecDeque::new();

            visit_queue.push_back(i);
            visited_cells_flags[i] = true;

            while let Some(current) = visit_queue.pop_front() {
                if board[current] == CellState::Flagged
                    && board
                        .neighbours_of(current, 1)
                        .any(|n| matches!(board[n], CellState::Revealed(_)))
                {
                    current_boundary.push(current);
                }

                visit_queue.extend(
                    board
                        .neighbours_of(current, 1)
                        .filter(|&n| {
                            board[n] == CellState::Flagged || board[n] == CellState::Hidden
                        })
                        .filter(|&n| !std::mem::replace(&mut visited_cells_flags[n], true)),
                )
            }
            mines_to_clear.push(*current_boundary.choose(rng).unwrap());
        }

        let mut mine_destinations = hidden_interior_cells
            .into_iter()
            .filter(|&h| board.content(h) != CellContent::Mine)
            .sample(rng, mines_to_clear.len());

        let empty_spots_needed = mines_to_clear.len() - mine_destinations.len();
        if empty_spots_needed > 0 {
            // push it into the revealed region and hope for the best
            mine_destinations.extend(
                (0..board.size())
                    .filter(|&i| {
                        matches!(
                            board[i],
                            CellState::Revealed(CellContent::Empty | CellContent::Number(_))
                        ) && !board.is_in_start_region(i)
                    })
                    .sample(rng, empty_spots_needed),
            );
        }

        log::debug!(
            "Handled {} enclosed regions. Pushed {} mines in and {} out.",
            mines_to_clear.len(),
            mine_destinations.len() - empty_spots_needed,
            empty_spots_needed
        );

        for (&s, &d) in mines_to_clear.iter().zip(mine_destinations.iter()) {
            board.move_mine(s, d);
        }

        mines_to_clear
            .into_iter()
            .chain(mine_destinations)
            .collect()
    }
}
