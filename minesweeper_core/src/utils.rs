use crate::board::{Board, CellState};
use std::collections::{HashSet, VecDeque};

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
pub fn find_disjoint_frontiers(board: &Board) -> (Vec<Vec<usize>>, HashSet<usize>) {
    let board_size = board.width * board.height;

    let mut frontier_union_set = HashSet::new();
    let mut nonfrontier_hidden_cells = HashSet::new();

    for idx in (0..board_size).filter(|&i| board[i] == CellState::Hidden) {
        let is_in_frontier = board
            .neighbours_of(idx, 1)
            .any(|j| matches!(board[j], CellState::Revealed(_)));

        if is_in_frontier {
            frontier_union_set.insert(idx);
        } else {
            nonfrontier_hidden_cells.insert(idx);
        }
    }

    // --- BFS to split the frontier into disjoint sets

    let mut visited_cells = HashSet::<usize>::new();
    let mut frontiers = Vec::<Vec<usize>>::new();

    for &i in frontier_union_set.iter() {
        if visited_cells.contains(&i) {
            continue;
        }

        let mut current_frontier = Vec::new();
        let mut visit_queue = VecDeque::new();

        visit_queue.push_back(i);

        while let Some(current) = visit_queue.pop_front() {
            if visited_cells.contains(&current) {
                continue;
            }
            current_frontier.push(current);
            visited_cells.insert(current);

            // find depth-2 neighbours in the frontier separated by a revealed num
            // i.e (hidden -> revealed num -> hidden)
            visit_queue.extend(
                board
                    .neighbours_of(current, 1)
                    .filter(|&n| matches!(board[n], CellState::Revealed(_)))
                    .flat_map(|n| board.neighbours_of(n, 1))
                    .filter(|next_hidden| {
                        frontier_union_set.contains(next_hidden)
                            && !visited_cells.contains(next_hidden)
                    }),
            )
        }

        frontiers.push(current_frontier);
    }

    (frontiers, nonfrontier_hidden_cells)
}
