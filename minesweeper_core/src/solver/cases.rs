use super::NeighbourInfo;
use crate::board::{Board, CellContent, CellState};

pub type CaseHandler = for<'a> fn(&mut Board, usize, &'a NeighbourInfo) -> Option<&'a Vec<usize>>;

/// Opens/flags every hidden neighbour of a revealed number based on whether the
/// number equals neighbouring mines OR hidden neighbours + flagged respectively.
///
/// ```text
///    F  ?     F  ?
///   (1)      (2)
/// ```
///
/// Returns the indices that were newly flagged/opened, or `None` if the rule
/// didn't apply.
pub fn handle_count_match_case<'a>(
    board: &mut Board,
    idx: usize,
    neighbour_info: &'a NeighbourInfo,
) -> Option<&'a Vec<usize>> {
    let CellState::Revealed(CellContent::Number(cell_number)) = board.state_grid[idx] else {
        return None;
    };

    if (neighbour_info.hidden.len() as u8) + neighbour_info.flagged_count == cell_number {
        for &cidx in &neighbour_info.hidden {
            board.toggle_cell_flag(cidx).unwrap();
        }
    } else if neighbour_info.flagged_count == cell_number {
        for &cidx in &neighbour_info.hidden {
            board.open_cell(cidx).unwrap();
        }
    } else {
        return None;
    }

    Some(&neighbour_info.hidden)
}
