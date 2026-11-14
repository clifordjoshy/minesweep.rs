use rand::prelude::{SeedableRng, SliceRandom, SmallRng};

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

pub struct BoardParams {
    pub width: usize,
    pub height: usize,
    pub mine_count: usize,
    pub start_idx: usize,
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
    revealed_count: usize,
    flagged_count: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub enum BoardError {
    BoardTooSmall,
    MineCountValidationError { min_mines: usize, max_mines: usize },
    InvalidPosition,
}

#[derive(Debug, PartialEq, Eq)]
pub enum BoardCellOpenResult {
    Noop,
    Revealed(Vec<usize>),
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
}

impl Board {
    pub fn size(&self) -> usize {
        self.width * self.height
    }

    pub fn neighbours_of(&self, idx: usize, radius: u8) -> impl Iterator<Item = usize> + use<> {
        let (width, height) = (self.width, self.height);
        let (x, y) = ((idx % width) as i32, (idx / width) as i32);

        let r = radius as i32;
        (-r..=r)
            .flat_map(move |dy| (-r..=r).map(move |dx| (dx, dy)))
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

    pub(crate) fn from_seed(params: &BoardParams, seed: u64) -> Result<Self, BoardError> {
        let BoardParams {
            width,
            height,
            mine_count,
            start_idx,
        } = *params;

        let total_cells = width * height;

        if total_cells < 16 {
            return Err(BoardError::BoardTooSmall);
        }

        if mine_count == 0 || mine_count >= total_cells - 8 {
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
            state_grid: vec![CellState::default(); total_cells].into_boxed_slice(),
            content_grid: vec![CellContent::default(); total_cells].into_boxed_slice(),
            revealed_count: 0,
            flagged_count: 0,
        };

        let definitely_not_mine_indices = board
            .neighbours_of(start_idx, 1)
            .chain(std::iter::once(start_idx))
            .collect::<Vec<_>>();

        let mut indices_to_place = (0..total_cells)
            .filter(|idx| !definitely_not_mine_indices.contains(idx))
            .collect::<Vec<_>>();
        let mut rng = SmallRng::seed_from_u64(seed);
        let mine_indices = &*indices_to_place.partial_shuffle(&mut rng, mine_count).0;

        for &midx in mine_indices {
            board.content_grid[midx] = CellContent::Mine;

            for nidx in board.neighbours_of(midx, 1) {
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
            CellContent::Number(_) => {
                self.revealed_count += 1;
                BoardCellOpenResult::Revealed(vec![open_idx])
            }
            CellContent::Empty => {
                // open all hidden neighbours (flood fill)
                let mut revealed_cells = vec![open_idx];
                let mut revealed_cells_iter = 0;

                while revealed_cells_iter < revealed_cells.len() {
                    let idx = revealed_cells[revealed_cells_iter];
                    revealed_cells_iter += 1;

                    if self.content_grid[idx] != CellContent::Empty {
                        continue;
                    }

                    for n in self.neighbours_of(idx, 1) {
                        if self.state_grid[n] == CellState::Hidden {
                            let cell_content = self.content_grid[n];
                            self.state_grid[n] = CellState::Revealed(cell_content);
                            revealed_cells.push(n);
                        }
                    }
                }

                self.revealed_count += revealed_cells.len();

                BoardCellOpenResult::Revealed(revealed_cells)
            }
        };

        if matches!(result, BoardCellOpenResult::Revealed(_)) && self.is_won() {
            // mark any pending cells as flagged
            for i in 0..self.state_grid.len() {
                if self.state_grid[i] == CellState::Hidden {
                    self.flag_cell(i).unwrap();
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
            .neighbours_of(open_idx, 1)
            .filter(|&nidx| self.state_grid[nidx] == CellState::Flagged)
            .count() as u8;

        if nearby_mines != marked_mines {
            return Ok(BoardCellOpenResult::Noop);
        }

        let mut open_res = BoardCellOpenResult::Noop;
        let mut revealed_cells = Vec::<usize>::new();

        for nidx in self.neighbours_of(open_idx, 1) {
            if self.state_grid[nidx] != CellState::Hidden {
                continue;
            }
            let nopen_res = self.open_cell(nidx).unwrap();
            if nopen_res.is_end_condition() {
                open_res = nopen_res;
                break;
            } else if let BoardCellOpenResult::Revealed(idxs) = nopen_res {
                revealed_cells.extend(idxs);
            }
        }

        Ok(if open_res.is_end_condition() {
            open_res
        } else if !revealed_cells.is_empty() {
            BoardCellOpenResult::Revealed(revealed_cells)
        } else {
            BoardCellOpenResult::Noop
        })
    }

    pub fn flag_cell(&mut self, flag_idx: usize) -> Result<(), BoardError> {
        if matches!(self.state(flag_idx)?, CellState::Revealed(_)) {
            return Err(BoardError::InvalidPosition);
        }

        if self.state_grid[flag_idx] == CellState::Hidden {
            self.state_grid[flag_idx] = CellState::Flagged;
            self.flagged_count += 1;
        }

        Ok(())
    }

    pub fn unflag_cell(&mut self, flag_idx: usize) -> Result<(), BoardError> {
        if matches!(self.state(flag_idx)?, CellState::Revealed(_)) {
            return Err(BoardError::InvalidPosition);
        }

        if self.state_grid[flag_idx] == CellState::Flagged {
            self.state_grid[flag_idx] = CellState::Hidden;
            self.flagged_count -= 1;
        }

        Ok(())
    }

    pub fn is_won(&self) -> bool {
        self.revealed_count == self.size() - self.mine_count
    }

    pub fn mines_remaining(&self) -> isize {
        self.mine_count as isize - self.flagged_count as isize
    }

    pub fn state(&self, idx: usize) -> Result<CellState, BoardError> {
        if idx >= self.state_grid.len() {
            return Err(BoardError::InvalidPosition);
        }
        Ok(self.state_grid[idx])
    }

    pub(crate) fn content(&self, idx: usize) -> CellContent {
        self.content_grid[idx]
    }

    fn rehide_cell(&mut self, idx: usize) {
        match self.state_grid[idx] {
            CellState::Hidden => return,
            CellState::Flagged => self.flagged_count -= 1,
            CellState::Revealed(_) => self.revealed_count -= 1,
        }

        self.state_grid[idx] = CellState::Hidden;
    }

    pub(crate) fn move_mine(&mut self, source_idx: usize, dest_idx: usize) {
        assert!(self.content_grid[source_idx] == CellContent::Mine);
        assert!(self.content_grid[dest_idx] != CellContent::Mine);

        self.content_grid[dest_idx] = CellContent::Mine;
        self.content_grid[source_idx] = match self
            .neighbours_of(source_idx, 1)
            .filter(|&i| i != dest_idx && self.content_grid[i] == CellContent::Mine)
            .count()
        {
            0 => CellContent::Empty,
            mc => CellContent::Number(mc as u8),
        };
        self.rehide_cell(source_idx);
        self.rehide_cell(dest_idx);

        for i in self.neighbours_of(source_idx, 1) {
            match self.content_grid[i] {
                CellContent::Mine => continue,
                CellContent::Number(n) if n > 1 => {
                    self.content_grid[i] = CellContent::Number(n - 1);
                }
                CellContent::Number(_) => self.content_grid[i] = CellContent::Empty,
                CellContent::Empty => unreachable!(),
            };
            self.rehide_cell(i);
        }
        for i in self.neighbours_of(dest_idx, 1) {
            match self.content_grid[i] {
                CellContent::Mine => continue,
                CellContent::Empty => self.content_grid[i] = CellContent::Number(1),
                CellContent::Number(n) => self.content_grid[i] = CellContent::Number(n + 1),
            };
            self.rehide_cell(i);
        }
    }

    pub fn reset_state(&mut self) {
        self.state_grid.fill(CellState::Hidden);
        self.flagged_count = 0;
        self.revealed_count = 0;
        self.open_cell(self.start_idx).unwrap();
    }
}

impl std::ops::Index<usize> for Board {
    type Output = CellState;

    fn index(&self, index: usize) -> &Self::Output {
        &self.state_grid[index]
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
