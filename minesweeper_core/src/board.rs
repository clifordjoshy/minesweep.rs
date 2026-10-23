use std::collections::HashSet;

use rand::prelude::{SeedableRng, SliceRandom, StdRng};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum CellContent {
    #[default]
    Empty,
    Mine,
    Number(u8), // Will hold values 1 through 8
}

/// The player's current interaction state with this tile.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
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
    pub start_idx: usize,
    state_grid: Box<[CellState]>,
    content_grid: Box<[CellContent]>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum BoardError {
    MineCountValidationError { min_mines: usize, max_mines: usize },
    InvalidPosition,
}

#[derive(Debug, PartialEq, Eq)]
pub enum BoardCellOpenResult {
    Noop,
    /// revealed the requested index only
    Revealed,
    /// revealed other indices than (along with) the requested one
    RevealedOther(HashSet<usize>),
    MineExploded,
    AllMinesSweeped,
}

impl BoardCellOpenResult {
    pub fn is_end_condition(&self) -> bool {
        matches!(
            self,
            BoardCellOpenResult::MineExploded | BoardCellOpenResult::AllMinesSweeped
        )
    }

    pub fn is_new_information(&self) -> bool {
        matches!(
            self,
            BoardCellOpenResult::Revealed | BoardCellOpenResult::RevealedOther(_)
        )
    }
}

impl Board {
    pub fn neighbours_of(&self, idx: usize) -> impl Iterator<Item = usize> + use<> {
        let (width, height) = (self.width, self.height);
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
        let total_cells = width * height;

        if mine_count == 0 || mine_count >= total_cells.saturating_sub(8) {
            return Err(BoardError::MineCountValidationError {
                min_mines: 1,
                max_mines: total_cells - 8, // technically should be total - neighbours, but good enough
            });
        }
        if start_idx >= total_cells {
            return Err(BoardError::InvalidPosition);
        }

        let mut board = Self {
            width,
            height,
            mine_count,
            start_idx,
            seed,
            state_grid: vec![CellState::default(); width * height].into_boxed_slice(),
            content_grid: vec![CellContent::default(); width * height].into_boxed_slice(),
        };

        let definitely_not_mine_indices = board
            .neighbours_of(start_idx)
            .chain(std::iter::once(start_idx))
            .collect::<Vec<_>>();

        let mut indices_to_place = (0..total_cells)
            .filter(|idx| !definitely_not_mine_indices.contains(idx))
            .collect::<Vec<_>>();

        let mut rng = StdRng::seed_from_u64(seed);
        indices_to_place.shuffle(&mut rng);

        let mine_indices = &indices_to_place[0..mine_count];

        for &midx in mine_indices {
            board.content_grid[midx] = CellContent::Mine;

            for nidx in board.neighbours_of(midx) {
                board.content_grid[nidx] = match board.content_grid[nidx] {
                    CellContent::Empty => CellContent::Number(1),
                    CellContent::Number(c) => CellContent::Number(c + 1),
                    CellContent::Mine => CellContent::Mine,
                };
            }
        }

        board.open_cell(start_idx).unwrap();

        Ok(board)
    }

    pub fn open_cell(&mut self, open_idx: usize) -> Result<BoardCellOpenResult, BoardError> {
        if self.state(open_idx)? != CellState::Hidden {
            return Ok(BoardCellOpenResult::Noop);
        }

        let content = self.content_grid[open_idx];
        self.state_grid[open_idx] = CellState::Revealed(content);

        let mut result = match content {
            CellContent::Mine => BoardCellOpenResult::MineExploded,
            CellContent::Number(_) => BoardCellOpenResult::Revealed,
            CellContent::Empty => {
                // open all hidden neighbours (flood fill)

                let mut all_cells_to_open = vec![open_idx];
                let mut cells_opened = HashSet::<usize>::new();

                while let Some(i) = all_cells_to_open.pop() {
                    let content = self.content_grid[i];
                    self.state_grid[i] = CellState::Revealed(content);
                    cells_opened.insert(i);

                    if content != CellContent::Empty {
                        continue;
                    }

                    all_cells_to_open.extend(self.neighbours_of(i).filter(|&n| {
                        self.state_grid[n] == CellState::Hidden && !cells_opened.contains(&n)
                    }));
                }

                BoardCellOpenResult::RevealedOther(cells_opened)
            }
        };

        if result.is_new_information() && self.is_won() {
            // mark any pending cells as flagged
            for i in 0..self.state_grid.len() {
                if self.state_grid[i] == CellState::Hidden {
                    self.state_grid[i] = CellState::Flagged;
                }
            }
            result = BoardCellOpenResult::AllMinesSweeped;
        }

        Ok(result)
    }

    pub fn chord_open_cell(&mut self, open_idx: usize) -> Result<BoardCellOpenResult, BoardError> {
        let CellState::Revealed(CellContent::Number(nearby_mines)) = self.state(open_idx)? else {
            return Ok(BoardCellOpenResult::Noop);
        };

        // open all neighbours that can be opened given current markings (chording)
        let marked_mines = self
            .neighbours_of(open_idx)
            .filter(|&nidx| self.state_grid[nidx] == CellState::Flagged)
            .count() as u8;

        if nearby_mines != marked_mines {
            return Ok(BoardCellOpenResult::Noop);
        }

        let mut cells_opened = HashSet::<usize>::new();
        let mut open_res = BoardCellOpenResult::Noop;

        for nidx in self.neighbours_of(open_idx) {
            if self.state_grid[nidx] != CellState::Hidden || cells_opened.contains(&nidx) {
                continue;
            }
            let nopen_res = self.open_cell(nidx).unwrap();
            if nopen_res.is_end_condition() {
                open_res = nopen_res;
                break;
            } else if nopen_res == BoardCellOpenResult::Revealed {
                cells_opened.insert(nidx);
            } else if let BoardCellOpenResult::RevealedOther(cv) = nopen_res {
                cells_opened.extend(cv);
            }
        }

        Ok(if open_res.is_end_condition() {
            open_res
        } else if !cells_opened.is_empty() {
            BoardCellOpenResult::RevealedOther(cells_opened)
        } else {
            BoardCellOpenResult::Noop
        })
    }

    pub fn flag_cell(&mut self, flag_idx: usize) -> Result<(), BoardError> {
        if matches!(self.state(flag_idx)?, CellState::Revealed(_)) {
            return Err(BoardError::InvalidPosition);
        }

        self.state_grid[flag_idx] = CellState::Flagged;

        Ok(())
    }

    pub fn unflag_cell(&mut self, flag_idx: usize) -> Result<(), BoardError> {
        if matches!(self.state(flag_idx)?, CellState::Revealed(_)) {
            return Err(BoardError::InvalidPosition);
        }

        self.state_grid[flag_idx] = CellState::Hidden;

        Ok(())
    }

    pub fn is_won(&self) -> bool {
        let revealed_cells = self
            .state_grid
            .iter()
            .filter(|s| matches!(s, CellState::Revealed(_)))
            .count();

        let safe_cells = self.width * self.height - self.mine_count;

        revealed_cells == safe_cells
    }

    pub fn mines_remaining(&self) -> isize {
        let flagged = self
            .state_grid
            .iter()
            .filter(|s| matches!(s, CellState::Flagged))
            .count();
        self.mine_count as isize - flagged as isize
    }

    pub fn state(&self, idx: usize) -> Result<CellState, BoardError> {
        if idx >= self.state_grid.len() {
            return Err(BoardError::InvalidPosition);
        }
        Ok(self.state_grid[idx])
    }

    pub fn state_iter(&self) -> impl Iterator<Item = (usize, &CellState)> {
        self.state_grid.iter().enumerate()
    }
}

impl std::fmt::Debug for Board {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Seed: {}", self.seed)?;
        writeln!(f, "State View ---------")?;
        for y in 0..self.height {
            for x in 0..self.width {
                let cell = &self.state_grid[y * self.width + x];

                match cell {
                    CellState::Hidden => write!(f, " X ")?,
                    CellState::Revealed(cc) => match cc {
                        CellContent::Empty => write!(f, "   ")?,
                        CellContent::Number(n) => write!(f, " {} ", n)?,
                        CellContent::Mine => write!(f, " * ")?,
                    },
                    CellState::Flagged => write!(f, " ~ ")?,
                }
            }
            writeln!(f)?;
        }

        writeln!(f, "Content View ---------")?;
        for y in 0..self.height {
            for x in 0..self.width {
                let cell = &self.content_grid[y * self.width + x];

                match cell {
                    CellContent::Empty => write!(f, "   ")?,
                    CellContent::Number(n) => write!(f, " {} ", n)?,
                    CellContent::Mine => write!(f, " * ")?,
                }
            }
            writeln!(f)?;
        }
        Ok(())
    }
}
