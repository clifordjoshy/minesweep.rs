use rand::prelude::{SeedableRng, SliceRandom, StdRng};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum CellContent {
    #[default]
    Empty,
    Mine,
    Number(u8), // Will hold values 1 through 8
}

/// The player's current interaction state with this tile.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum CellState {
    #[default]
    Hidden,
    Revealed(CellContent),
    Flagged,
}

#[derive(Clone)]
pub struct Board {
    pub width: usize,
    pub height: usize,
    pub mine_count: usize,
    pub seed: u64,
    pub state_grid: Box<[CellState]>,
    content_grid: Box<[CellContent]>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum BoardError {
    MineCountValidationError { min_mines: usize, max_mines: usize },
    InvalidPosition,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BoardCellOpenResult {
    Noop,
    RevealedNum,
    MineExploded,
    AllMinesSweeped,
}

impl Board {
    pub fn neighbour_iter(idx: usize, width: usize, height: usize) -> impl Iterator<Item = usize> {
        let (x, y) = ((idx % width) as i32, (idx / width) as i32);

        (-1..=1)
            .flat_map(move |dy| (-1..=1).map(move |dx| (dx, dy)))
            .filter_map(move |(dx, dy)| {
                if dx == 0 && dy == 0 {
                    return None;
                }
                let nx = x + dx;
                let ny = y + dy;

                if nx < 0 || nx >= width as i32 || ny < 0 || ny >= height as i32 {
                    return None;
                }

                Some((ny * width as i32 + nx) as usize)
            })
    }

    pub fn new(
        width: usize,
        height: usize,
        mine_count: usize,
        start_idx: usize,
    ) -> Result<Self, BoardError> {
        let seed = rand::random();
        Self::from_seed(width, height, mine_count, start_idx, seed)
    }

    pub fn from_seed(
        width: usize,
        height: usize,
        mine_count: usize,
        start_idx: usize,
        seed: u64,
    ) -> Result<Self, BoardError> {
        let total_cells = width.strict_mul(height);
        if mine_count == 0 || mine_count >= total_cells - 8 {
            return Err(BoardError::MineCountValidationError {
                min_mines: 0,
                max_mines: total_cells - 8, // technically should be total - neighbours, but good enough
            });
        }
        if start_idx >= total_cells {
            return Err(BoardError::InvalidPosition);
        }

        let definitely_not_mine_indices = Board::neighbour_iter(start_idx, width, height)
            .chain(std::iter::once(start_idx))
            .collect::<Vec<_>>();

        let mut content_grid = vec![CellContent::default(); width * height].into_boxed_slice();
        let state_grid = vec![CellState::default(); width * height].into_boxed_slice();

        let mut indices_to_place = (0..total_cells)
            .filter(|idx| !definitely_not_mine_indices.contains(idx))
            .collect::<Vec<_>>();

        let mut rng = StdRng::seed_from_u64(seed);
        indices_to_place.shuffle(&mut rng);

        let mine_indices = &indices_to_place[0..mine_count];

        for &midx in mine_indices {
            content_grid[midx] = CellContent::Mine;

            for nidx in Board::neighbour_iter(midx, width, height) {
                content_grid[nidx] = match content_grid[nidx] {
                    CellContent::Empty => CellContent::Number(1),
                    CellContent::Number(c) => CellContent::Number(c + 1),
                    CellContent::Mine => CellContent::Mine,
                };
            }
        }

        let mut board = Self {
            width,
            height,
            mine_count,
            seed,
            state_grid,
            content_grid,
        };
        board.open_cell(start_idx).unwrap();

        Ok(board)
    }

    pub fn open_cell(&mut self, open_idx: usize) -> Result<BoardCellOpenResult, BoardError> {
        if open_idx >= self.state_grid.len() {
            return Err(BoardError::InvalidPosition);
        }

        let mut result = match self.state_grid[open_idx] {
            CellState::Hidden => {
                let content = self.content_grid[open_idx];
                self.state_grid[open_idx] = CellState::Revealed(content);

                match content {
                    CellContent::Mine => BoardCellOpenResult::MineExploded,
                    CellContent::Number(_) => BoardCellOpenResult::RevealedNum,
                    CellContent::Empty => {
                        // open all hidden neighbours
                        let mut open_res = BoardCellOpenResult::Noop;
                        for nidx in Board::neighbour_iter(open_idx, self.width, self.height) {
                            if self.state_grid[nidx] != CellState::Hidden {
                                continue;
                            }
                            open_res = std::cmp::max(self.open_cell(nidx).unwrap(), open_res);
                        }
                        open_res
                    }
                }
            }
            CellState::Flagged => BoardCellOpenResult::Noop,
            CellState::Revealed(CellContent::Number(nearby_mines)) => {
                // open all neighbours that can be opened given current markings aka chording

                let marked_mines = Board::neighbour_iter(open_idx, self.width, self.height)
                    .map(|nidx| (self.state_grid[nidx] == CellState::Flagged) as u8)
                    .sum::<u8>();

                let mut open_res = BoardCellOpenResult::Noop;
                if nearby_mines == marked_mines {
                    for nidx in Board::neighbour_iter(open_idx, self.width, self.height) {
                        if self.state_grid[nidx] != CellState::Hidden {
                            continue;
                        }
                        open_res = std::cmp::max(self.open_cell(nidx).unwrap(), open_res);
                    }
                }

                open_res
            }
            CellState::Revealed(CellContent::Empty) => BoardCellOpenResult::Noop,
            _ => unreachable!("A user shouldn't be able to open any revealed non-number cell."),
        };

        if result == BoardCellOpenResult::RevealedNum && self.is_won() {
            // mark any pending cells as flagged
            for i in 0..self.state_grid.len() {
                if self.state_grid[i] == CellState::Hidden {
                    self.state_grid[i] = CellState::Flagged;
                }
            }
            result = BoardCellOpenResult::AllMinesSweeped;
        } else if result == BoardCellOpenResult::MineExploded {
            // reveal all mines
            for i in 0..self.state_grid.len() {
                if self.content_grid[i] == CellContent::Mine
                    && self.state_grid[i] == CellState::Hidden
                {
                    self.state_grid[i] = CellState::Revealed(CellContent::Mine);
                }
            }
        }

        Ok(result)
    }

    pub fn toggle_cell_flag(&mut self, flag_idx: usize) -> Result<(), BoardError> {
        if flag_idx >= self.state_grid.len() {
            return Err(BoardError::InvalidPosition);
        }

        match self.state_grid[flag_idx] {
            CellState::Revealed(_) => {}
            CellState::Hidden => {
                self.state_grid[flag_idx] = CellState::Flagged;
            }
            CellState::Flagged => {
                self.state_grid[flag_idx] = CellState::Hidden;
            }
        }

        Ok(())
    }

    pub fn is_won(&mut self) -> bool {
        let revealed_cells = self
            .state_grid
            .iter()
            .filter(|s| matches!(s, CellState::Revealed(_)))
            .count();

        let safe_cells = self.width * self.height - self.mine_count;

        revealed_cells == safe_cells
    }
}
