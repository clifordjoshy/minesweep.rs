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

pub struct Board {
    pub width: usize,
    pub height: usize,
    pub mine_count: usize,
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
}

impl Board {
    #[inline]
    fn neighbour_iter(idx: usize, width: usize, height: usize) -> impl Iterator<Item = usize> {
        (-1..=1)
            .flat_map(move |dy| (-1..=1).map(move |dx| (dx, dy)))
            .filter_map(move |(dx, dy)| {
                if dx == 0 && dy == 0 {
                    return None;
                }
                let target_idx = (idx as i32) + dx + (width as i32) * dy;
                let max_size = (width * height) as i32;
                if target_idx < 0 || target_idx >= max_size {
                    return None;
                }

                Some(target_idx as usize)
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

        let state_grid = (0..total_cells)
            .map(|idx| {
                if definitely_not_mine_indices.contains(&idx) {
                    CellState::Revealed(content_grid[idx])
                } else {
                    CellState::Hidden
                }
            })
            .collect::<Box<[_]>>();

        Ok(Self {
            width,
            height,
            mine_count,
            state_grid,
            content_grid,
        })
    }

    pub fn open_cell(&mut self, open_idx: usize) -> Result<BoardCellOpenResult, BoardError> {
        if open_idx > self.state_grid.len() {
            return Err(BoardError::InvalidPosition);
        }

        match self.state_grid[open_idx] {
            CellState::Hidden => {
                let content = self.content_grid[open_idx];
                self.state_grid[open_idx] = CellState::Revealed(content);

                match content {
                    CellContent::Mine => Ok(BoardCellOpenResult::MineExploded),
                    CellContent::Number(_) => Ok(BoardCellOpenResult::RevealedNum),
                    CellContent::Empty => {
                        // open all hidden neighbours
                        let mut open_res = BoardCellOpenResult::Noop;
                        for nidx in Board::neighbour_iter(open_idx, self.width, self.height) {
                            if self.state_grid[nidx] != CellState::Hidden {
                                continue;
                            }
                            open_res = std::cmp::max(self.open_cell(nidx).unwrap(), open_res);
                        }
                        Ok(open_res)
                    }
                }
            }
            CellState::Flagged => Ok(BoardCellOpenResult::Noop),
            CellState::Revealed(CellContent::Number(nearby_mines)) => {
                // open all neighbours that can be opened given current markings

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

                Ok(BoardCellOpenResult::Noop)
            }
            _ => unreachable!("A user shouldn't be able to open any revealed non-number cell."),
        }
    }

    pub fn flag_cell(&mut self, flag_idx: usize) -> Result<(), BoardError> {
        if flag_idx > self.state_grid.len() {
            return Err(BoardError::InvalidPosition);
        }

        self.state_grid[flag_idx] = CellState::Flagged;

        return Ok(());
    }

    pub fn print(&self) {
        for y in 0..self.height {
            for x in 0..self.width {
                match self.state_grid[y * self.width + x] {
                    CellState::Hidden => print!(" X "),
                    CellState::Revealed(cc) => match cc {
                        CellContent::Empty => print!("   "),
                        CellContent::Number(n) => print!(" {} ", n),
                        CellContent::Mine => print!(" # ",),
                    },
                    CellState::Flagged => print!(" ~ "),
                }
            }
            println!()
        }
        println!("---------");
    }
}
