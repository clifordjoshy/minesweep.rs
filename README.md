<p align="center"> <img src="gui/assets/icon.svg" alt="Minesweeper icon" width="96" height="96"> </p> <h1 align="center">minesweep.rs</h1>

A Minesweeper game where every board can be solved by pure logic, with no guessing required. Inspired by the Minesweeper in [Simon Tatham's Portable Puzzle Collection](https://www.chiark.greenend.org.uk/~sgtatham/puzzles/). Built in Rust with [egui](https://github.com/emilk/egui).

**[Play in your browser](https://cliford.net/demos/minesweeper)** (WebAssembly build)

<p align="center">
  <img src="./images/screenshot.png" alt="screenshot" height="350">
</p>

## How to play

- **Left click** to reveal a cell
- **Right click** to flag or unflag a cell
- Numbers show how many mines are in the neighbouring cells
- Reveal every safe cell to win

## Download & Installation

### Pre-built Binaries

Standalone executables for **Windows** and **Linux** (`x86_64`) are available on the [Releases](../../releases) page.

### Build from Source

#### Quick Run

```sh
cargo run --release
```

#### Linux Desktop Integration

To build and install the binary (`minesweep_rs`), desktop entry, and icon system-wide (defaults to `/usr/local`):

```sh
make build
sudo make install
```

To install for the current user only (without `sudo`):

```sh
make install PREFIX=~/.local
```

To uninstall:

```sh
sudo make uninstall
```

### Web (WASM)

Using [Trunk](https://trunkrs.dev):

```sh
trunk serve --release
```

Then open `http://127.0.0.1:8080` in your browser.

## More Links

- https://cliford.net/projects/minesweeper
