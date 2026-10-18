pub const APP_NAME: &str = "Minesweeper.rs";

/// Side length of one cell in screen points.
pub const CELL_SIZE: f32 = 28.0;

/// The three standard Minesweeper presets.
#[derive(Clone, Copy, PartialEq, Eq, Default, serde::Deserialize, serde::Serialize)]
pub enum Difficulty {
    #[default]
    Beginner,
    Intermediate,
    Expert,
}

impl Difficulty {
    pub const ALL: [Difficulty; 3] = [
        Difficulty::Beginner,
        Difficulty::Intermediate,
        Difficulty::Expert,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Difficulty::Beginner => "Beginner (9x9, 10)",
            Difficulty::Intermediate => "Intermediate (16x16, 40)",
            Difficulty::Expert => "Expert (30x16, 99)",
        }
    }

    /// Returns `(width, height, mine_count)`.
    pub fn params(self) -> (usize, usize, usize) {
        match self {
            Difficulty::Beginner => (9, 9, 10),
            Difficulty::Intermediate => (16, 16, 40),
            Difficulty::Expert => (30, 16, 99),
        }
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
