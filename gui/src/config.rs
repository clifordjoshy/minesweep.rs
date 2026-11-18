use egui::Color32;
use minesweeper_core::generator;

pub const APP_NAME: &str = "minesweep.rs";

pub const DEFAULT_THEME: egui::Theme = egui::Theme::Dark;

pub const DEFAULT_DIFFICULTY: Difficulty = Difficulty::Intermediate;

/// Side length of one cell in screen points.
pub const CELL_SIZE: f32 = 28.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum Difficulty {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum BoardType {
    Simple,
    Solvable,
}

impl BoardType {
    pub const ALL: [BoardType; 2] = [BoardType::Simple, BoardType::Solvable];

    pub fn label(self) -> String {
        format!("{:?}", self)
    }

    pub fn generator(self) -> &'static dyn generator::BoardGenerator {
        match self {
            BoardType::Simple => &generator::SimpleBoardGenerator,
            BoardType::Solvable => &generator::SolvableBoardGenerator,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GameStatus {
    Waiting,
    Playing,
    Won,
    Lost,
}

impl GameStatus {
    pub fn label(self) -> &'static str {
        match self {
            GameStatus::Waiting => "Click any cell to start.",
            GameStatus::Playing => "Good luck!",
            GameStatus::Won => "You win!",
            GameStatus::Lost => "Boom! You hit a mine.",
        }
    }
}

pub struct Colors {
    // cells
    pub hidden: Color32,
    pub hidden_hover: Color32,
    pub revealed: Color32,
    pub incorrect: Color32, // revealed number cell with wrong flags around it
    pub mine_bg: Color32,
    pub mine_glyph: Color32,
    // numbers 1..=8 (index 0 is "1")
    pub numbers: [Color32; 8],
    // flag
    pub flag_pole: Color32, // also used for the base
    pub flag_cloth: Color32,
    // status line
    pub won: Color32,
    pub lost: Color32,
}

pub const LIGHT_MODE: Colors = Colors {
    hidden: Color32::from_gray(178),
    hidden_hover: Color32::from_gray(196),
    revealed: Color32::from_gray(226),
    incorrect: Color32::from_rgb(255, 200, 200),
    mine_bg: Color32::from_rgb(215, 80, 80),
    mine_glyph: Color32::BLACK,
    numbers: [
        Color32::from_rgb(0, 0, 255),   // 1
        Color32::from_rgb(0, 110, 0),   // 2
        Color32::from_rgb(190, 0, 0), // 3 (darker than pure red so it reads on the pink "incorrect" bg)
        Color32::from_rgb(0, 0, 128), // 4
        Color32::from_rgb(128, 0, 0), // 5
        Color32::from_rgb(0, 110, 110), // 6
        Color32::BLACK,               // 7
        Color32::from_gray(90),       // 8
    ],
    flag_pole: Color32::from_gray(25),
    flag_cloth: Color32::from_rgb(195, 19, 19),
    won: Color32::from_rgb(0, 100, 0),
    lost: Color32::from_rgb(180, 0, 0),
};

pub const DARK_MODE: Colors = Colors {
    hidden: Color32::from_gray(85),
    hidden_hover: Color32::from_gray(110),
    revealed: Color32::from_gray(48),
    incorrect: Color32::from_rgb(90, 30, 30),
    mine_bg: Color32::from_rgb(170, 50, 50),
    mine_glyph: Color32::WHITE,
    numbers: [
        Color32::from_rgb(110, 160, 255), // 1
        Color32::from_rgb(80, 200, 100),  // 2
        Color32::from_rgb(255, 105, 105), // 3
        Color32::from_rgb(160, 140, 255), // 4
        Color32::from_rgb(255, 160, 80),  // 5
        Color32::from_rgb(70, 205, 205),  // 6
        Color32::from_gray(225),          // 7
        Color32::from_gray(165),          // 8
    ],
    flag_pole: Color32::from_gray(225),
    flag_cloth: Color32::from_rgb(230, 60, 60),
    won: Color32::from_rgb(90, 220, 110),
    lost: Color32::from_rgb(255, 110, 110),
};

impl Colors {
    pub fn for_theme(theme: egui::Theme) -> &'static Colors {
        match theme {
            egui::Theme::Dark => &DARK_MODE,
            egui::Theme::Light => &LIGHT_MODE,
        }
    }

    pub fn number(&self, n: u8) -> Color32 {
        self.numbers
            .get((n as usize).wrapping_sub(1))
            .copied()
            .unwrap_or(Color32::GRAY)
    }
}

pub const DIGITS_TEXT: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];

pub const MINE_TEXT: &str = "*";
