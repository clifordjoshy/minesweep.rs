pub const APP_NAME: &str = "minesweep.rs";

/// Side length of one cell in screen points.
pub const CELL_SIZE: f32 = 28.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Deserialize, serde::Serialize)]
pub enum Difficulty {
    #[default]
    Beginner,
    Intermediate,
    Expert,
    Master,
}

impl Difficulty {
    pub const ALL: [Difficulty; 4] = [
        Difficulty::Beginner,
        Difficulty::Intermediate,
        Difficulty::Expert,
        Difficulty::Master,
    ];

    /// Returns `(width, height, mine_count)`.
    pub fn params(self) -> (usize, usize, usize) {
        match self {
            Difficulty::Beginner => (10, 10, 10),
            Difficulty::Intermediate => (16, 16, 40),
            Difficulty::Expert => (30, 16, 99),
            Difficulty::Master => (30, 24, 200),
        }
    }

    pub fn label(self) -> String {
        let (w, h, m) = self.params();
        format!("{:?} ({}x{}, {})", self, w, h, m)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Deserialize, serde::Serialize)]

pub enum BoardType {
    Simple,
    #[default]
    Solvable,
}

impl BoardType {
    pub const ALL: [BoardType; 2] = [BoardType::Simple, BoardType::Solvable];

    pub fn label(self) -> String {
        format!("{:?}", self)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum GameStatus {
    #[default]
    Ready,
    Playing,
    Won,
    Lost,
}

impl GameStatus {
    pub fn label(self) -> &'static str {
        match self {
            GameStatus::Ready => "Click any cell to start (right-click to flag)",
            GameStatus::Playing => "Good luck!",
            GameStatus::Won => "You win!",
            GameStatus::Lost => "Boom! You hit a mine.",
        }
    }
}
