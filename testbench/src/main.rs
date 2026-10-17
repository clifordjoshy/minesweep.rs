use minesweeper_core::*;

fn main() {
    let mut board = Board::from_seed(10, 10, 30, 15, 10).unwrap();
    println!("---------");
    board.print();
    board.open_cell(17);
    board.flag_cell(27);
    board.open_cell(17);
    board.print();
}
