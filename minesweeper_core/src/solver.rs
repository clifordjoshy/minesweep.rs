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

pub fn solve_board(board: &mut Board) -> SolverResult {
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
        if let Some(modified_indices) = handle_frontier_backtracking_case(board) {
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
    let (frontiers, non_frontier) = find_disjoint_frontiers(board);

    let total_mines_remaining = board.mines_remaining().max(0) as usize;
    let max_mines_per_frontier = total_mines_remaining - frontiers.len() + 1;

    let all_frontiers_size = frontiers.iter().map(|f| f.len()).sum::<usize>();

    // accumulate all constraints in the frontier so that can test quickly
    let get_constraints = |frontier: &Vec<usize>| {
        frontier
            .iter()
            .flat_map(|&fidx| board.neighbours_of(fidx))
            .collect::<HashSet<_>>()
            .into_iter()
            .filter_map(|nidx| {
                let CellState::Revealed(CellContent::Number(cell_num)) = board.state(nidx).unwrap()
                else {
                    return None;
                };

                let flagged_count = board
                    .neighbours_of(nidx)
                    .filter(|&nnidx| board.state(nnidx).unwrap() == CellState::Flagged)
                    .count() as u8;

                let mines_needed = cell_num - flagged_count;

                let frontier_neighbour_mask = board
                    .neighbours_of(nidx)
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

    if min_mines_guaranteed_in_entire_frontier == total_mines_remaining {
        definitely_not_mines.extend(non_frontier);
    } else if max_mines_possible_in_entire_frontier
        == total_mines_remaining.saturating_sub(non_frontier.len())
    {
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
        if res == BoardCellOpenResult::Revealed {
            modified_indices.push(idx);
        } else if let BoardCellOpenResult::RevealedOther(indices) = res {
            modified_indices.extend(indices);
        }
    }

    if modified_indices.is_empty() {
        None
    } else {
        Some(modified_indices)
    }
}

/// Helper method to split the board into disjoint sets of hidden cells that
/// are mutually independent in terms of constraints.
/// The returned frontier vecs are in traversal order, so that backtracking can
/// happens with locality
fn find_disjoint_frontiers(board: &Board) -> (Vec<Vec<usize>>, HashSet<usize>) {
    let board_size = board.width * board.height;

    let mut frontier_union_set = HashSet::new();
    let mut nonfrontier_hidden_cells = HashSet::new();

    for idx in (0..board_size).filter(|&i| board.state(i).unwrap() == CellState::Hidden) {
        let is_in_frontier = board
            .neighbours_of(idx)
            .any(|j| matches!(board.state(j).unwrap(), CellState::Revealed(_)));

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
                    .neighbours_of(current)
                    .filter(|&n| matches!(board.state(n).unwrap(), CellState::Revealed(_)))
                    .flat_map(|n| board.neighbours_of(n))
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
