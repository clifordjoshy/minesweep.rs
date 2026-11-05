use crate::board::{Board, CellState};
use std::collections::VecDeque;

pub struct NeighbourInfo {
    cell_idx: usize,
    hidden_bitmask: u8,
    flagged_bitmask: u8,
}

impl NeighbourInfo {
    pub fn hidden_count(&self) -> u8 {
        self.hidden_bitmask.count_ones() as u8
    }
    pub fn flagged_count(&self) -> u8 {
        self.flagged_bitmask.count_ones() as u8
    }
    pub fn hidden<'a>(&'a self, board: &Board) -> impl Iterator<Item = usize> + use<'a> {
        board
            .neighbours_of(self.cell_idx, 1)
            .enumerate()
            .filter_map(|(niter, board_idx)| {
                if (self.hidden_bitmask & (1u8 << niter)) != 0 {
                    Some(board_idx)
                } else {
                    None
                }
            })
    }

    pub fn for_cell(board: &Board, idx: usize) -> Self {
        let mut hidden_bitmask = 0u8;
        let mut flagged_bitmask = 0u8;

        for (niter, board_idx) in board.neighbours_of(idx, 1).enumerate() {
            let niter_mask = 1u8 << niter;
            match board[board_idx] {
                CellState::Hidden => hidden_bitmask |= niter_mask,
                CellState::Flagged => flagged_bitmask |= niter_mask,
                CellState::Revealed(_) => {}
            }
        }

        Self {
            cell_idx: idx,
            hidden_bitmask,
            flagged_bitmask,
        }
    }
}

/// Helper method to split the board into disjoint sets of hidden cells that
/// are mutually independent in terms of constraints.
/// The returned frontier vecs are in traversal order, so that backtracking can
/// happens with locality
pub fn find_disjoint_frontiers(board: &Board) -> (Vec<Vec<usize>>, Vec<usize>) {
    let board_size = board.size();

    let mut frontier_flags = vec![false; board_size];
    let mut nonfrontier_flags = vec![false; board_size];

    for idx in (0..board_size).filter(|&i| board[i] == CellState::Hidden) {
        let is_in_frontier = board
            .neighbours_of(idx, 1)
            .any(|j| matches!(board[j], CellState::Revealed(_)));

        if is_in_frontier {
            frontier_flags[idx] = true;
        } else {
            nonfrontier_flags[idx] = true;
        }
    }

    // --- BFS to split the frontier into disjoint sets

    let mut visited_cells_flags = vec![false; board_size];
    let mut frontiers = Vec::<Vec<usize>>::new();

    for i in (0..board_size).filter(|&i| frontier_flags[i]) {
        if visited_cells_flags[i] {
            continue;
        }

        let mut current_frontier = Vec::new();
        let mut visit_queue = VecDeque::new();

        visit_queue.push_back(i);

        while let Some(current) = visit_queue.pop_front() {
            if visited_cells_flags[current] {
                continue;
            }
            current_frontier.push(current);
            visited_cells_flags[current] = true;

            // find depth-2 neighbours in the frontier separated by a revealed num
            // i.e (hidden -> revealed num -> hidden)
            visit_queue.extend(
                board
                    .neighbours_of(current, 1)
                    .filter(|&n| matches!(board[n], CellState::Revealed(_)))
                    .flat_map(|n| board.neighbours_of(n, 1))
                    .filter(|&next_hidden| {
                        frontier_flags[next_hidden] && !visited_cells_flags[next_hidden]
                    }),
            )
        }

        frontiers.push(current_frontier);
    }

    let non_frontier = (0..board_size)
        .filter(|&i| nonfrontier_flags[i])
        .collect::<Vec<_>>();

    (frontiers, non_frontier)
}
