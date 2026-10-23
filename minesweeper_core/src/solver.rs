use crate::{
    BoardCellOpenResult, CellContent,
    board::{Board, CellState},
    solver::SolverResult::Solved,
};
use std::collections::{BTreeSet, HashSet, VecDeque};

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
    Unsolvable,
    Solved,
}

pub fn solve_board(mut board: &mut Board) -> SolverResult {
    let board_size = board.width * board.height;

    // queue of indices that might be relevant (neighbours of any actioned index)
    let mut relevant_queue: VecDeque<usize> = VecDeque::new();

    'solve_outer: loop {
        // iter index we use when the relevant queue is empty
        // after a full scan of the table, we try the fallback
        let mut manual_iter_index = 0;

        let mut process_index;
        'check_simple_cases: loop {
            if manual_iter_index == board_size {
                break;
            } else if !relevant_queue.is_empty() {
                // reset manual iter
                process_index = relevant_queue.pop_front().unwrap();
                manual_iter_index = 0;
            } else {
                process_index = manual_iter_index;
                manual_iter_index += 1;
            }

            if !matches!(board.state(process_index).unwrap(), CellState::Revealed(_)) {
                continue;
            }

            let neighbour_info = neighbour_counts(board, process_index);

            if neighbour_info.hidden.is_empty() {
                continue;
            }

            for case_op in [handle_count_match_case, handle_neighbour_constraining_case] {
                if let Some(modified_indices) = case_op(board, process_index, &neighbour_info) {
                    relevant_queue.extend(
                        modified_indices
                            .into_iter()
                            .flat_map(|i| std::iter::once(i).chain(board.neighbours_of(i)))
                            .collect::<HashSet<_>>()
                            .into_iter()
                            .filter(|&i| matches!(board.state(i).unwrap(), CellState::Revealed(_))),
                    );
                    continue 'check_simple_cases;
                }
            }
        }

        if board.is_won() {
            return Solved;
        }

        // we did a full iteration of the board with no progress on the simple
        // cases. let's try the fallback case and hope for some progress
        if let Some(modified_indices) = handle_frontier_backtracking_case(&mut board) {
            relevant_queue.extend(
                modified_indices
                    .into_iter()
                    .filter(|&i| matches!(board.state(i).unwrap(), CellState::Revealed(_))),
            );
            continue 'solve_outer;
        } else {
            // nothing else to do now
            return SolverResult::Unsolvable;
        }
    }
}

/// Opens/flags every hidden neighbour of a revealed number based on whether the
/// number equals flagged OR hidden neighbours + flagged respectively.
///
/// ```text
///    F  ?     F  ?
///   (1)      (2)
/// ```
///
/// Returns the indices that were newly flagged/opened, or `None` if the rule
/// didn't apply.
fn handle_count_match_case(
    board: &mut Board,
    idx: usize,
    neighbour_info: &NeighbourInfo,
) -> Option<Vec<usize>> {
    let CellState::Revealed(CellContent::Number(cell_number)) = board.state(idx).unwrap() else {
        return None;
    };

    let mut modified_indices = Vec::<usize>::new();

    if (neighbour_info.hidden.len() as u8) + neighbour_info.flagged_count == cell_number {
        for &cidx in &neighbour_info.hidden {
            board.flag_cell(cidx).unwrap();
            modified_indices.push(cidx);
        }
    } else if neighbour_info.flagged_count == cell_number {
        for &cidx in &neighbour_info.hidden {
            let open_res = board.open_cell(cidx).unwrap();
            // sanity check
            assert_ne!(open_res, BoardCellOpenResult::MineExploded);

            if let BoardCellOpenResult::RevealedOther(idxs) = open_res {
                modified_indices.extend(idxs);
            } else if open_res == BoardCellOpenResult::Revealed {
                modified_indices.push(cidx);
            }
        }
    } else {
        return None;
    }

    Some(modified_indices)
}

/// Handles cases where a particular revealed cell is constrained by a revealed
/// neighbour. This allows us to flag the cell not affected by the constraining
/// neighbour. Or open cells if the constraining neighbours guarantee it isn't
/// a mine. For a given revealed cell, its hidden neighbours can be *directly*
/// affected by depth 2 neighbours (see third example below).
///
/// ```text
///  ?   ?   ?    |   F   ?   ?   ?   |   (1) ?
///  0  (2) (1)   |   0  (2) (1)  ?   |       ? (2)
///  0   0   0    |   0   0   0   ?   |       ?
/// ```
///
/// Returns the indices that were newly flagged/opened, or `None` if the rule
/// didn't apply.
fn handle_neighbour_constraining_case(
    board: &mut Board,
    idx: usize,
    neighbour_info: &NeighbourInfo,
) -> Option<Vec<usize>> {
    let CellState::Revealed(CellContent::Number(current_cell_number)) = board.state(idx).unwrap()
    else {
        return None;
    };

    let cur_region = &neighbour_info.hidden;
    let num_mines_in_cur_region = current_cell_number.checked_sub(neighbour_info.flagged_count)?;

    let mut definitely_mines = HashSet::<usize>::new();
    let mut definitely_not_mines = HashSet::<usize>::new();

    // track regions unaffected by constraints
    // for a final deduction from the complement of all constraints
    let mut region_left = cur_region.clone();
    let mut mines_left_max_possible = num_mines_in_cur_region;
    let mut mines_left_min_guaranteed = num_mines_in_cur_region;

    let affecting_neighbours = board
        .neighbours_of(idx)
        .flat_map(|nidx| board.neighbours_of(nidx))
        .filter(|&nidx| nidx != idx)
        .collect::<BTreeSet<_>>();

    for nidx in affecting_neighbours.into_iter() {
        let CellState::Revealed(CellContent::Number(neighbour_num)) = board.state(nidx).unwrap()
        else {
            continue;
        };

        let ncounts = neighbour_counts(board, nidx);
        let nbr_region = &ncounts.hidden;
        let num_mines_in_nbr_region = neighbour_num.checked_sub(ncounts.flagged_count)?;

        let cur_intersection_nbr = cur_region
            .intersection(nbr_region)
            .copied()
            .collect::<HashSet<_>>();

        let cur_complement_nbr = cur_region
            .difference(&cur_intersection_nbr)
            .copied()
            .collect::<HashSet<_>>();

        // in this case, we use the intersecting region to deduce information about
        // the complement region. so if we don't have both, noop
        if cur_intersection_nbr.is_empty() || cur_complement_nbr.is_empty() {
            continue;
        }

        // minimum mines in the intersection is when every cell of the neighbour
        // that's not intersecting (complement) is a mine.
        let min_mines_in_intersection = num_mines_in_nbr_region
            .saturating_sub((nbr_region.len() - cur_intersection_nbr.len()) as u8);

        let max_mines_in_intersection =
            std::cmp::min(cur_intersection_nbr.len() as u8, num_mines_in_nbr_region);

        // single-constraint deductions
        let are_all_mines_in_intersection = num_mines_in_cur_region == min_mines_in_intersection;

        let are_all_mines_in_complement = num_mines_in_cur_region
            .saturating_sub(max_mines_in_intersection)
            == cur_complement_nbr.len() as u8;

        if are_all_mines_in_intersection {
            definitely_not_mines.extend(cur_complement_nbr);
        } else if are_all_mines_in_complement {
            definitely_mines.extend(cur_complement_nbr);
        }

        // accumulate for the multi-constraint deduction
        mines_left_min_guaranteed =
            mines_left_min_guaranteed.saturating_sub(max_mines_in_intersection);
        if cur_intersection_nbr.is_subset(&region_left) {
            mines_left_max_possible =
                mines_left_max_possible.saturating_sub(min_mines_in_intersection);
        }
        region_left.retain(|c| !cur_intersection_nbr.contains(c));
    }

    // multi-constraint deductions on whatever the intersections didn't cover
    if !region_left.is_empty() && region_left.len() < cur_region.len() {
        if mines_left_min_guaranteed == region_left.len() as u8 {
            definitely_mines.extend(region_left);
        } else if mines_left_max_possible == 0 {
            definitely_not_mines.extend(region_left);
        }
    }

    if definitely_mines.is_empty() && definitely_not_mines.is_empty() {
        return None;
    }

    let mut modified_indices = Vec::<usize>::new();

    for &i in definitely_mines.iter() {
        board.flag_cell(i).unwrap();
    }

    modified_indices.extend(definitely_mines);

    for &i in definitely_not_mines.iter() {
        let open_res = board.open_cell(i).unwrap();
        // sanity check
        assert_ne!(open_res, BoardCellOpenResult::MineExploded);

        if let BoardCellOpenResult::RevealedOther(idxs) = open_res {
            modified_indices.extend(idxs);
        }
    }

    modified_indices.extend(definitely_not_mines);

    Some(modified_indices)
}

/// Worst case fallback when the above two strategies are unable to make progress.
/// Nothing too smart, just backtracking on the "frontier" (disjoint sets of all
/// revealed cells neighbouring a hidden cell)
/// - Will handle deductions from constraints spanning two revealed cells that
///   don't directly overlap in their neighbourhood.
/// - Also takes the total mine count into account for end-game deductions.
pub fn handle_frontier_backtracking_case(board: &mut Board) -> Option<Vec<usize>> {
    // todo!();
    return None;
}
