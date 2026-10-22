use std::collections::HashSet;

use super::NeighbourInfo;
use crate::{
    BoardCellOpenResult,
    board::{Board, CellContent, CellState},
    solver::neighbour_counts,
};

pub type CaseHandler = fn(&mut Board, usize, &NeighbourInfo) -> Option<Vec<usize>>;

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
pub fn handle_count_match_case(
    board: &mut Board,
    idx: usize,
    neighbour_info: &NeighbourInfo,
) -> Option<Vec<usize>> {
    let CellState::Revealed(CellContent::Number(cell_number)) = board.state(idx).unwrap() else {
        return None;
    };

    if (neighbour_info.hidden.len() as u8) + neighbour_info.flagged_count == cell_number {
        for &cidx in &neighbour_info.hidden {
            board.flag_cell(cidx).unwrap();
        }
    } else if neighbour_info.flagged_count == cell_number {
        for &cidx in &neighbour_info.hidden {
            let open_res = board.open_cell(cidx).unwrap();
            // sanity check
            assert_ne!(open_res, BoardCellOpenResult::MineExploded);
        }
    } else {
        return None;
    }

    Some(neighbour_info.hidden.iter().copied().collect::<Vec<_>>())
}

/// Handles cases where a particular revealed cell is constrained by a revealed
/// neighbour. This allows us to flag the cell not affected by the constraining
/// neighbour. Or the opposite.
///
/// ```text
///  ?   ?   ?          F   ?   ?   ?
///  0  (2) (1)         0  (2) (1)  ?
///  0   0   0          0   0   0   ?
/// ```
///
/// Returns the indices that were newly flagged/opened, or `None` if the rule
/// didn't apply.
pub fn handle_neighbour_constraining_case(
    board: &mut Board,
    idx: usize,
    neighbour_info: &NeighbourInfo,
) -> Option<Vec<usize>> {
    let CellState::Revealed(CellContent::Number(current_cell_number)) = board.state(idx).unwrap()
    else {
        return None;
    };

    let cur_region = &neighbour_info.hidden;
    let num_mines_in_cur_region = current_cell_number - neighbour_info.flagged_count;

    let mut definitely_mines = HashSet::<usize>::new();
    let mut definitely_not_mines = HashSet::<usize>::new();

    // track regions unaffected by constraints
    // for a final deduction from the complement of all constraints
    let mut region_left = cur_region.clone();
    let mut mines_left_max_possible = num_mines_in_cur_region;
    let mut mines_left_min_guaranteed = num_mines_in_cur_region;

    for &nidx in &neighbour_info.revealed {
        let CellState::Revealed(CellContent::Number(neighbour_num)) = board.state(nidx).unwrap()
        else {
            continue;
        };

        let ncounts = neighbour_counts(board, nidx);
        let nbr_region = &ncounts.hidden;
        let num_mines_in_nbr_region = neighbour_num - ncounts.flagged_count;

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

    for &i in definitely_mines.iter() {
        board.flag_cell(i).unwrap();
    }
    for &i in definitely_not_mines.iter() {
        let open_res = board.open_cell(i).unwrap();
        // sanity check
        assert_ne!(open_res, BoardCellOpenResult::MineExploded);
    }

    let modified_indices = definitely_mines
        .into_iter()
        .chain(definitely_not_mines)
        .collect();

    Some(modified_indices)
}
