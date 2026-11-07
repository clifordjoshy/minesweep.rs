use crate::{
    BoardCellOpenResult, CellContent,
    board::{Board, CellState},
    utils,
};
use std::collections::VecDeque;

pub enum SolverResult {
    Unsolvable,
    Solved,
}

pub fn solve_board(board: &mut Board) -> SolverResult {
    // queue of indices that might be relevant (neighbours of any actioned index)
    let mut relevant_queue: VecDeque<usize> = VecDeque::new();

    // list of flags that indicate whether each index is in the queue (used to dedup)
    let mut in_queue: Vec<bool> = vec![false; board.size()];

    'solve_outer: loop {
        // iter index we use when the relevant queue is empty
        // after a full scan of the table, we try the fallback
        let mut manual_iter_index = 0;

        'check_simple_cases: loop {
            let process_index = if let Some(i) = relevant_queue.pop_front() {
                in_queue[i] = false;
                manual_iter_index = 0;
                i
            } else if manual_iter_index < board.size() {
                manual_iter_index += 1;
                manual_iter_index - 1
            } else {
                break;
            };

            if !matches!(board[process_index], CellState::Revealed(_)) {
                continue;
            }

            let neighbour_info = utils::NeighbourInfo::for_cell(board, process_index);

            if neighbour_info.hidden_count() == 0 {
                continue;
            }

            for case_op in [handle_count_match_case, handle_neighbour_constraining_case] {
                if let Some(modified_indices) = case_op(board, process_index, &neighbour_info) {
                    relevant_queue.extend(
                        modified_indices
                            .into_iter()
                            .flat_map(|i| std::iter::once(i).chain(board.neighbours_of(i, 2)))
                            .filter(|&i| matches!(board[i], CellState::Revealed(_)))
                            .filter(|&i| !std::mem::replace(&mut in_queue[i], true)),
                    );
                    continue 'check_simple_cases;
                }
            }
        }

        if board.is_won() {
            return SolverResult::Solved;
        }

        // we did a full iteration of the board with no progress on the simple
        // cases. let's try the fallback case and hope for some progress
        if let Some(modified_indices) = handle_frontier_backtracking_case(board) {
            relevant_queue.extend(
                modified_indices
                    .into_iter()
                    .flat_map(|i| std::iter::once(i).chain(board.neighbours_of(i, 2)))
                    .filter(|&i| matches!(board[i], CellState::Revealed(_)))
                    .filter(|&i| !std::mem::replace(&mut in_queue[i], true)),
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
    neighbour_info: &utils::NeighbourInfo,
) -> Option<Vec<usize>> {
    let CellState::Revealed(CellContent::Number(cell_number)) = board[idx] else {
        return None;
    };

    let mut modified_indices = Vec::<usize>::new();

    if neighbour_info.hidden_count() + neighbour_info.flagged_count() == cell_number {
        for cidx in neighbour_info.hidden(board) {
            board.flag_cell(cidx).unwrap();
            modified_indices.push(cidx);
        }
    } else if neighbour_info.flagged_count() == cell_number {
        for cidx in neighbour_info.hidden(board) {
            let open_res = board.open_cell(cidx).unwrap();
            assert_ne!(open_res, BoardCellOpenResult::MineExploded);

            if let BoardCellOpenResult::Revealed(idxs) = open_res {
                modified_indices.extend(idxs);
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
    neighbour_info: &utils::NeighbourInfo,
) -> Option<Vec<usize>> {
    let CellState::Revealed(CellContent::Number(cur_cell_number)) = board[idx] else {
        return None;
    };

    // all operations here will be in the 5x5 square around the `idx` cell
    // all referenes to "region" will use an u32 bitmask indicating the indices
    // of this 25 (at most) sized array (with the LSB being index 0)
    let board_width = board.width as isize;
    let region_bitmask_for_idx = |check_idx: usize| {
        let dx = (check_idx as isize % board_width) - (idx as isize % board_width);
        let dy = (check_idx as isize / board_width) - (idx as isize / board_width);
        if dx.abs() <= 2 && dy.abs() <= 2 {
            let box_index = (dy + 2) * 5 + (dx + 2);
            1u32 << box_index
        } else {
            0
        }
    };

    let cur_region = neighbour_info
        .hidden(board)
        .map(region_bitmask_for_idx)
        .fold(0, |m1, m2| m1 | m2);

    let num_mines_in_cur_region = cur_cell_number.checked_sub(neighbour_info.flagged_count())?;

    let mut definitely_mines = 0u32;
    let mut definitely_not_mines = 0u32;

    // track regions unaffected by constraints
    // for a final deduction from the complement of all constraints
    let mut cur_region_left = cur_region;
    let mut mines_left_max_possible = num_mines_in_cur_region;
    let mut mines_left_min_guaranteed = num_mines_in_cur_region;

    for nidx in board.neighbours_of(idx, 2) {
        let CellState::Revealed(CellContent::Number(neighbour_num)) = board[nidx] else {
            continue;
        };

        let nbinfo = utils::NeighbourInfo::for_cell(board, nidx);
        let nbr_region = nbinfo
            .hidden(board)
            .map(region_bitmask_for_idx)
            .fold(0, |m1, m2| m1 | m2);
        let num_mines_in_nbr_region = neighbour_num - nbinfo.flagged_count();

        let cur_intersection_nbr = cur_region & nbr_region;
        let cur_complement_nbr = cur_region & !nbr_region;

        // in this case, we use the intersecting region to deduce information about
        // the complement region. so if we don't have both, noop
        if cur_intersection_nbr == 0 || cur_complement_nbr == 0 {
            continue;
        }

        // minimum mines in the intersection is when every cell of the neighbour
        // that's not intersecting (complement) is a mine.
        let min_mines_in_intersection = num_mines_in_nbr_region
            .saturating_sub(nbinfo.hidden_count() - cur_intersection_nbr.count_ones() as u8);

        let max_mines_in_intersection = std::cmp::min(
            cur_intersection_nbr.count_ones() as u8,
            num_mines_in_nbr_region,
        );

        // single-constraint deductions
        let are_all_mines_in_intersection = num_mines_in_cur_region == min_mines_in_intersection;

        let are_all_mines_in_complement = num_mines_in_cur_region
            .saturating_sub(max_mines_in_intersection)
            == cur_complement_nbr.count_ones() as u8;

        if are_all_mines_in_intersection {
            definitely_not_mines |= cur_complement_nbr;
        } else if are_all_mines_in_complement {
            definitely_mines |= cur_complement_nbr;
        }

        // accumulate for the multi-constraint deduction
        mines_left_min_guaranteed =
            mines_left_min_guaranteed.saturating_sub(max_mines_in_intersection);

        // if the intersection is a subset of the region left
        if cur_intersection_nbr & cur_region_left == cur_intersection_nbr {
            mines_left_max_possible =
                mines_left_max_possible.saturating_sub(min_mines_in_intersection);
        }
        cur_region_left &= !cur_intersection_nbr;
    }

    // multi-constraint deductions on whatever the intersections didn't cover
    if cur_region_left > 0 && cur_region_left.count_ones() < cur_region.count_ones() {
        if mines_left_min_guaranteed == cur_region_left.count_ones() as u8 {
            definitely_mines |= cur_region_left;
        } else if mines_left_max_possible == 0 {
            definitely_not_mines |= cur_region_left;
        }
    }

    if definitely_mines == 0 && definitely_not_mines == 0 {
        return None;
    }

    let mut modified_indices = Vec::<usize>::new();

    for nidx in board.neighbours_of(idx, 1) {
        let n_mask = region_bitmask_for_idx(nidx);
        if definitely_mines & n_mask != 0 {
            board.flag_cell(nidx).unwrap();
            modified_indices.push(nidx);
        } else if definitely_not_mines & n_mask != 0 {
            let open_res = board.open_cell(nidx).unwrap();
            assert_ne!(open_res, BoardCellOpenResult::MineExploded);

            if let BoardCellOpenResult::Revealed(idxs) = open_res {
                modified_indices.extend(idxs);
            }
        }
    }

    Some(modified_indices)
}

/// Worst case fallback when the above two strategies are unable to make progress.
/// Nothing too smart, just backtracking on the "frontier" (disjoint sets of all
/// revealed cells neighbouring a hidden cell)
/// - Will handle deductions from constraints spanning two revealed cells that
///   don't directly overlap in their neighbourhood.
/// - Also takes the total mine count into account for end-game deductions.
fn handle_frontier_backtracking_case(board: &mut Board) -> Option<Vec<usize>> {
    let (frontiers, non_frontier) = utils::find_disjoint_frontiers(board);

    let total_mines_remaining = board.mines_remaining().max(0) as usize;
    let max_mines_per_frontier = total_mines_remaining - frontiers.len() + 1;

    let all_frontiers_size = frontiers.iter().map(|f| f.len()).sum::<usize>();

    // accumulate all constraints in the frontier so that can test quickly
    let get_constraints = |frontier: &[usize]| {
        let mut seen = vec![false; board.size()];

        frontier
            .iter()
            .flat_map(|&fidx| board.neighbours_of(fidx, 1))
            .filter(|&i| !std::mem::replace(&mut seen[i], true))
            .filter_map(|nidx| {
                let CellState::Revealed(CellContent::Number(cell_num)) = board[nidx] else {
                    return None;
                };

                let flagged_count = utils::NeighbourInfo::for_cell(board, nidx).flagged_count();
                let mines_needed = cell_num - flagged_count;

                let frontier_neighbour_mask = board
                    .neighbours_of(nidx, 1)
                    .filter_map(|nnidx| {
                        frontier
                            .iter()
                            .position(|&f| f == nnidx)
                            .map(|fidx| 1u128 << fidx)
                    })
                    .fold(0u128, |a, b| a | b);

                Some((frontier_neighbour_mask, mines_needed))
            })
            .collect::<Vec<_>>()
    };

    let mut definitely_mines = Vec::<usize>::new();
    let mut definitely_not_mines = Vec::<usize>::new();

    let mut min_mines_guaranteed_in_entire_frontier = 0;
    let mut max_mines_possible_in_entire_frontier = 0;

    for frontier in frontiers {
        if frontier.len() > 127 {
            // we might be able to go higher. this limit is just coming from
            // the bitmask being u128, but 127 seems like a good enough depth

            max_mines_possible_in_entire_frontier += frontier.len();

            continue;
        }

        let constraints = get_constraints(&frontier);

        let min_required_mines_in_frontier = total_mines_remaining
            .saturating_sub(non_frontier.len())
            .saturating_sub(all_frontiers_size - frontier.len());

        let mut min_guaranteed_mines_in_frontier = frontier.len();
        let mut max_possible_mines_in_frontier = 0;

        let full_mask = (1u128 << frontier.len()) - 1;

        let mut found_something = false;
        let mut definitely_mines_bitmask = full_mask;
        let mut definitely_not_mines_bitmask = full_mask;

        // essentially a depth first search on all possible combinations of
        // this frontier (using a bitmask to represent entries in the frontier
        // that's being treated as a mine)

        // we start by adding all children of the 0 (no mines in frontier) case
        // we know that there must be atleast one mine in this frontier since
        // we're in this function).

        // the tree looks something like below (if no pruning)
        //         000
        //   001   010      100
        //         011   101   110
        //                     111
        //
        // at the end of the traversal, if one cell is a mine across all possible
        // solutions, then we flag it. if not a mine across all possible solutions
        // we open it.
        // we also track the min and max possible mines across the frontier so that
        // we can make deductions about the non frontier region.

        let mut mine_bitmask_stack = (0..frontier.len()).map(|i| 1u128 << i).collect::<Vec<_>>();

        'next_bitmask: while let Some(mine_bitmask) = mine_bitmask_stack.pop() {
            if mine_bitmask.count_ones() as usize > max_mines_per_frontier {
                continue;
            }

            // the highest set bit tells us up to how many indexes from frontier
            // must already be confirmed. if that configuration isn't valid
            // then no point branching from there.
            let bits_already_set = mine_bitmask.ilog2();
            let must_be_fine_mask = (1u128 << (bits_already_set + 1)) - 1;
            let unchecked_mask = full_mask & !must_be_fine_mask;

            let mut all_constraints_satisfied = true;

            for &(frontier_neighbour_mask, mines_needed) in constraints.iter() {
                let flagged_in_constraint_region =
                    (mine_bitmask & frontier_neighbour_mask).count_ones() as u8;
                let unknown_in_constraint_region =
                    (frontier_neighbour_mask & unchecked_mask).count_ones() as u8;

                if flagged_in_constraint_region + unknown_in_constraint_region < mines_needed
                    || flagged_in_constraint_region > mines_needed
                {
                    // this configuration doesn't have enough / has too many mines
                    // in the `must_be_fine_mask` region.
                    // no point proceeding further with the children of this configuration
                    continue 'next_bitmask;
                }

                all_constraints_satisfied =
                    all_constraints_satisfied && flagged_in_constraint_region == mines_needed
            }

            if all_constraints_satisfied {
                if (mine_bitmask & full_mask).count_ones() as usize
                    >= min_required_mines_in_frontier
                {
                    // add this to the list of known solutions
                    found_something = true;
                    definitely_mines_bitmask &= mine_bitmask;
                    definitely_not_mines_bitmask &= full_mask & !mine_bitmask;

                    min_guaranteed_mines_in_frontier = std::cmp::min(
                        min_guaranteed_mines_in_frontier,
                        mine_bitmask.count_ones() as usize,
                    );
                    max_possible_mines_in_frontier = std::cmp::max(
                        max_possible_mines_in_frontier,
                        mine_bitmask.count_ones() as usize,
                    )
                }

                continue 'next_bitmask;
            }

            mine_bitmask_stack.extend(
                ((bits_already_set as usize + 1)..frontier.len())
                    .map(|i| mine_bitmask | (1u128 << i)),
            );
        }

        if found_something {
            for (frontier_idx, &board_idx) in frontier.iter().enumerate() {
                let idx_mask = 1u128 << frontier_idx;
                if definitely_mines_bitmask & idx_mask != 0 {
                    definitely_mines.push(board_idx);
                }
                if definitely_not_mines_bitmask & idx_mask != 0 {
                    definitely_not_mines.push(board_idx);
                }
            }

            min_mines_guaranteed_in_entire_frontier += min_guaranteed_mines_in_frontier;
            max_mines_possible_in_entire_frontier += max_possible_mines_in_frontier;
        } else {
            max_mines_possible_in_entire_frontier += frontier.len();
        }
    }

    if total_mines_remaining == min_mines_guaranteed_in_entire_frontier {
        definitely_not_mines.extend(non_frontier);
    } else if total_mines_remaining == max_mines_possible_in_entire_frontier + non_frontier.len() {
        definitely_mines.extend(non_frontier);
    }

    let mut modified_indices = Vec::new();

    for idx in definitely_mines {
        board.flag_cell(idx).unwrap();
        modified_indices.push(idx);
    }
    for idx in definitely_not_mines {
        let res = board.open_cell(idx).unwrap();
        assert_ne!(res, BoardCellOpenResult::MineExploded);
        if let BoardCellOpenResult::Revealed(idxs) = res {
            modified_indices.extend(idxs);
        }
    }

    (!modified_indices.is_empty()).then_some(modified_indices)
}
