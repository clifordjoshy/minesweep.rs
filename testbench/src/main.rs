// my_runner/src/main.rs

fn main() {
    // Call the function from your library package
    let message = minesweeper_core::greet("Developer");

    println!("{}", message);
}
