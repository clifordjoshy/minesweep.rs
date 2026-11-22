use crate::{
    BoardCellOpenResult, CellContent,
    board::{Board, CellState},
    utils,
};
use std::collections::VecDeque;

#[derive(Debug, PartialEq, Eq)]
pub enum SolverResult {
    Unsolvable,
    Solved,
}

pub fn solve_board(board: &mut Board, start_from: &[usize]) -> SolverResult {
    // queue of indices that might be relevant (neighbours of any actioned index)
    let mut relevant_queue = VecDeque::<usize>::new();

    // list of flags that indicate whether each index is in the queue (used to dedup)
    let mut in_queue: Vec<bool> = vec![false; board.size()];

    macro_rules! enqueue_relevant_indices {
        ($idxs:expr) => {
            relevant_queue.extend(
                $idxs
                    .iter()
                    .flat_map(|&i| std::iter::once(i).chain(board.neighbours_of(i, 2)))
                    .filter(|&i| matches!(board[i], CellState::Revealed(_)))
                    .filter(|&i| !std::mem::replace(&mut in_queue[i], true)),
            );
        };
    }

    enqueue_relevant_indices!(start_from);

    'solve_outer: loop {
        if board.is_won() {
            return SolverResult::Solved;
        }

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

            let neighbour_info = utils::NeighbourInfo::for_cell(board, process_index);

            if neighbour_info.hidden_count() == 0 {
                continue;
            }

            for case_op in [handle_count_match_case, handle_neighbour_constraining_case] {
                if let Some(modified_indices) = case_op(board, process_index, &neighbour_info) {
                    enqueue_relevant_indices!(modified_indices);
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
            enqueue_relevant_indices!(modified_indices);
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
            assert_ne!(
                open_res,
                BoardCellOpenResult::MineExploded,
                "Opened {} and lost it all. Was processing {idx} : {:?}",
                nidx,
                board
            );

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
    let (mut frontiers, interior) = utils::find_disjoint_frontiers(board);

    let any_frontiers_ignored = {
        // This limit is just coming from the bitmask being u128, but 127 seems like
        // a good enough depth.
        let len_before = frontiers.len();
        frontiers.retain(|f| f.len() < 128);
        frontiers.len() < len_before
    };

    let board_mines_remaining = board.mines_remaining().max(0) as usize;

    let constraints = {
        let mut board_idx_to_frontier_idx = vec![None::<usize>; board.size()];
        for f in &frontiers {
            for (fidx, &bidx) in f.iter().enumerate() {
                board_idx_to_frontier_idx[bidx] = Some(fidx);
            }
        }
        let mut seen = vec![false; board.size()];
        frontiers
            .iter()
            .map(|ftr| {
                ftr.iter()
                    .flat_map(|&i| board.neighbours_of(i, 1))
                    .filter(|&i| !std::mem::replace(&mut seen[i], true))
                    .filter_map(|nidx| {
                        let CellState::Revealed(CellContent::Number(cell_num)) = board[nidx] else {
                            return None;
                        };

                        let flagged_count =
                            utils::NeighbourInfo::for_cell(board, nidx).flagged_count();
                        let mines_needed = cell_num - flagged_count;

                        let frontier_neighbour_mask = board
                            .neighbours_of(nidx, 1)
                            .filter_map(|nnidx| board_idx_to_frontier_idx[nnidx])
                            .fold(0u128, |mask, fidx| mask | (1u128 << fidx));

                        Some((frontier_neighbour_mask, mines_needed))
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    };

    // min and max mines per frontier
    let bounds_per_frontier = {
        constraints
            .iter()
            .map(|cs| {
                let mut mask_union = 0u128;
                let mut min_mines = 0usize;
                let mut max_mines = 0usize;
                for &(cmask, cmines) in cs.iter() {
                    // min is when all possible mines are in overlap
                    min_mines +=
                        cmines.saturating_sub((cmask & mask_union).count_ones() as u8) as usize;

                    // max is when all possible mines are in complement
                    max_mines += cmines.min((cmask & !mask_union).count_ones() as u8) as usize;

                    mask_union |= cmask;
                }

                max_mines = max_mines.min(board_mines_remaining);

                (min_mines, max_mines)
            })
            .collect::<Vec<_>>()
    };

    let (mut min_mines_total, mut max_mines_total) = bounds_per_frontier
        .iter()
        .fold((0, 0), |(a, b), &(x, y)| ((a + x), (b + y)));

    // if the total number of mines we need is within the entire frontier's bounds
    // then it might be possible that a tighter bound (obtained by checking every
    // solution) might allow us to deduce information about the interior
    let attempt_deduction_on_totals = !any_frontiers_ignored
        && !interior.is_empty()
        && ((min_mines_total..=max_mines_total).contains(&board_mines_remaining)
            || (min_mines_total + interior.len()..=max_mines_total + interior.len())
                .contains(&board_mines_remaining));

    // if the above field is false, we can prune a bunch of checks
    // 1. all we plan to find is the list of cells that's a mine/nonmine always
    // 2. as soon as we get one valid configuration, we can skip checking any
    //    branch that does not refute the existing configurations
    // if the above is true, then we would need to check every possible configuration
    // so that we can tighten the bounds

    let mut definitely_mines = Vec::<usize>::new();
    let mut definitely_not_mines = Vec::<usize>::new();

    for ((frontier, constraints), &(min_mines_estimate, max_mines_estimate)) in frontiers
        .iter()
        .zip(constraints.iter())
        .zip(bounds_per_frontier.iter())
    {
        let mut min_mines_actual = max_mines_estimate;
        let mut max_mines_actual = min_mines_estimate;

        let full_mask = (1u128 << frontier.len()) - 1;

        let mut definitely_mines_mask = full_mask;
        let mut definitely_not_mines_mask = full_mask;
        let mut found_any = false;

        // essentially a depth first search on all possible combinations of
        // this frontier (using a bitmask to represent entries in the frontier
        // that's being treated as a mine)
        // the tree looks something like below (if no pruning)
        //         000
        //   001   010      100
        //         011   101   110
        //                     111
        //
        // we start by adding all children from depth 1 into the stack
        // at the end of the traversal, if one cell is a mine across all possible
        // solutions, then we flag it. if not a mine across all possible solutions
        // we open it.

        let max_mines_allowed = board_mines_remaining
            .saturating_sub(min_mines_total - min_mines_estimate)
            .min(max_mines_estimate);
        let min_mines_required = if !any_frontiers_ignored {
            board_mines_remaining
                .saturating_sub(interior.len() + max_mines_total - max_mines_estimate)
        } else {
            0
        };

        let mut mine_mask_stack = (0..frontier.len()).map(|i| 1u128 << i).collect::<Vec<_>>();

        'next_bitmask: while let Some(mine_mask) = mine_mask_stack.pop() {
            // the highest set bit tells us up to how many indexes from frontier
            // must already be decided.
            let bits_already_set = mine_mask.ilog2();
            let decided_mask = (1u128 << (bits_already_set + 1)) - 1;
            let undecided_mask = full_mask & !decided_mask;

            if !attempt_deduction_on_totals
                // if this mask does not refute the accumulated masks and
                // the children of this mask will not refute the acc masks
                && (definitely_not_mines_mask & (mine_mask | undecided_mask) == 0
                    && definitely_mines_mask & !mine_mask == 0)
            {
                continue 'next_bitmask;
            }

            let mut all_constraints_satisfied = true;

            for &(cmask, nmines) in constraints.iter() {
                let flagged = (mine_mask & cmask).count_ones() as u8;
                let unknown = (cmask & undecided_mask).count_ones() as u8;

                if flagged + unknown < nmines || flagged > nmines {
                    // this configuration doesn't have enough / has too many mines
                    // in the `decided_mask` region. pruned
                    continue 'next_bitmask;
                }

                all_constraints_satisfied = all_constraints_satisfied && flagged == nmines
            }

            if all_constraints_satisfied {
                if !(min_mines_required..=max_mines_allowed)
                    .contains(&(mine_mask.count_ones() as usize))
                {
                    // while this is a valid solution, it does not fit in with
                    // the mine counts that we have
                    continue 'next_bitmask;
                }

                found_any = true;
                // add this to the list of known solutions
                definitely_mines_mask &= mine_mask;
                definitely_not_mines_mask &= !mine_mask;

                if attempt_deduction_on_totals {
                    min_mines_actual = min_mines_actual.min(mine_mask.count_ones() as usize);
                    max_mines_actual = max_mines_actual.max(mine_mask.count_ones() as usize)
                }
            } else if (mine_mask.count_ones() as usize) < max_mines_allowed {
                mine_mask_stack.extend(
                    ((bits_already_set as usize + 1)..frontier.len())
                        .map(|i| mine_mask | 1u128 << i),
                );
            }
        }

        if !found_any {
            continue;
        }

        for (frontier_idx, &board_idx) in frontier.iter().enumerate() {
            let idx_mask = 1u128 << frontier_idx;
            if definitely_mines_mask & idx_mask != 0 {
                definitely_mines.push(board_idx);
            }
            if definitely_not_mines_mask & idx_mask != 0 {
                definitely_not_mines.push(board_idx);
            }
        }

        if attempt_deduction_on_totals {
            // tighten the bounds with actual limits seen
            min_mines_total += min_mines_actual - min_mines_estimate;
            max_mines_total -= max_mines_estimate - max_mines_actual;
        }
    }

    if attempt_deduction_on_totals {
        if board_mines_remaining == min_mines_total {
            definitely_not_mines.extend(interior);
        } else if board_mines_remaining == max_mines_total + interior.len() {
            definitely_mines.extend(interior);
        }
    }

    let mut modified_indices = Vec::new();

    for idx in definitely_mines {
        board.flag_cell(idx).unwrap();
        modified_indices.push(idx);
    }
    for idx in definitely_not_mines {
        match board.open_cell(idx).unwrap() {
            BoardCellOpenResult::Noop => {}
            BoardCellOpenResult::Revealed(idxs) => modified_indices.extend(idxs),
            BoardCellOpenResult::AllMinesSweeped => modified_indices.push(idx),
            BoardCellOpenResult::MineExploded => unreachable!(),
        }
    }

    (!modified_indices.is_empty()).then_some(modified_indices)
}
