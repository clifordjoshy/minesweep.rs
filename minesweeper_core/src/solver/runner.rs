use super::cases::*;
use crate::board::{Board, CellState};
use std::collections::{HashSet, VecDeque};

#[derive(Debug, Default)]
pub struct NeighbourInfo {
    pub hidden: HashSet<usize>,
    pub flagged_count: u8,
    pub revealed: HashSet<usize>,
}

pub fn neighbour_counts(board: &Board, idx: usize) -> NeighbourInfo {
    let mut neighbour_info = NeighbourInfo::default();

    for nidx in board.neighbours_of(idx) {
        match board.state(nidx).unwrap() {
            CellState::Hidden => _ = neighbour_info.hidden.insert(nidx),
            CellState::Flagged => neighbour_info.flagged_count += 1,
            CellState::Revealed(_) => _ = neighbour_info.revealed.insert(nidx),
        }
    }

    neighbour_info
}

pub enum SolverResult {
    Unsolvable(Board),
    Solved(Board),
}

const CASE_CHECKS: [CaseHandler; 2] = [handle_count_match_case, handle_neighbour_constraining_case];

pub fn solve_board(board: &Board) -> SolverResult {
    let mut board = board.clone();
    let board_size = board.width * board.height;

    // queue of indices that might be relevant (neighbours of any actioned index)
    let mut relevant_queue: VecDeque<usize> = VecDeque::new();

    // if queue is empty, we just iterate over the table and hope for the best
    // one full run of the table with no new entries in the relevant queue
    // would likely imply unsolvability
    let mut steps_without_progress = 0;
    let mut process_index = 0;

    'outer: loop {
        if steps_without_progress == board_size {
            return SolverResult::Unsolvable(board);
        } else if board.is_won() {
            return SolverResult::Solved(board);
        }

        if let Some(i) = relevant_queue.pop_front() {
            process_index = i;
            steps_without_progress = 0;
        } else {
            process_index = (process_index + 1) % board_size;
            steps_without_progress += 1;
        }

        if !matches!(board.state(process_index).unwrap(), CellState::Revealed(_)) {
            continue;
        }

        let neighbour_info = neighbour_counts(&board, process_index);

        if neighbour_info.hidden.is_empty() {
            continue;
        }

        for case_op in CASE_CHECKS {
            if let Some(modified_indices) = case_op(&mut board, process_index, &neighbour_info) {
                relevant_queue.extend(modified_indices.iter());
                relevant_queue.extend(
                    modified_indices
                        .iter()
                        .flat_map(|&i| board.neighbours_of(i)),
                );
                continue 'outer;
            }
        }
    }
}
