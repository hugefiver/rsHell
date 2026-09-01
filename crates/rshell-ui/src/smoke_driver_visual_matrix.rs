use crate::{ShellLayout, ShellLayoutMode};

#[path = "smoke_driver_visual_platform.rs"]
mod platform;
pub use platform::visual_matrix;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SmokeVisualState {
    Empty,
    Connected,
    TwentyTabs,
    Single,
    HSplit,
    VSplit,
    TopBottom3,
    Grid,
    Editor,
    Settings,
    Import,
    HostKey,
    Authentication,
    Failure,
    Recovery,
}

impl SmokeVisualState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Connected => "connected",
            Self::TwentyTabs => "twenty_tabs",
            Self::Single => "single",
            Self::HSplit => "h_split",
            Self::VSplit => "v_split",
            Self::TopBottom3 => "top_bottom_3",
            Self::Grid => "grid",
            Self::Editor => "editor",
            Self::Settings => "settings",
            Self::Import => "import",
            Self::HostKey => "host_key",
            Self::Authentication => "authentication",
            Self::Failure => "failure",
            Self::Recovery => "recovery",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|state| state.as_str() == value)
    }

    pub const ALL: [Self; 15] = [
        Self::Empty,
        Self::Connected,
        Self::TwentyTabs,
        Self::Single,
        Self::HSplit,
        Self::VSplit,
        Self::TopBottom3,
        Self::Grid,
        Self::Editor,
        Self::Settings,
        Self::Import,
        Self::HostKey,
        Self::Authentication,
        Self::Failure,
        Self::Recovery,
    ];
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmokeVisualCheckpoint {
    pub id: String,
    pub state: SmokeVisualState,
    pub width: i32,
    pub height: i32,
    pub expected_mode: ShellLayoutMode,
}

impl SmokeVisualCheckpoint {
    pub fn validate(&self) -> bool {
        valid_checkpoint_id(&self.id)
            && supported_dimensions(self.width, self.height)
            && ShellLayout::for_width(self.width).mode == self.expected_mode
    }
}

pub const fn supported_dimensions(width: i32, height: i32) -> bool {
    matches!(
        (width, height),
        (800, 600) | (1_000, 700) | (1_360, 860) | (1_920, 1_080)
    )
}

pub const REQUIRED_SMOKE_VISUAL_MATRIX: [(i32, i32, SmokeVisualState, ShellLayoutMode); 26] = [
    (800, 600, SmokeVisualState::Empty, ShellLayoutMode::Compact),
    (
        800,
        600,
        SmokeVisualState::TwentyTabs,
        ShellLayoutMode::Compact,
    ),
    (800, 600, SmokeVisualState::Grid, ShellLayoutMode::Compact),
    (800, 600, SmokeVisualState::Editor, ShellLayoutMode::Compact),
    (
        800,
        600,
        SmokeVisualState::Settings,
        ShellLayoutMode::Compact,
    ),
    (800, 600, SmokeVisualState::Import, ShellLayoutMode::Compact),
    (
        800,
        600,
        SmokeVisualState::Recovery,
        ShellLayoutMode::Compact,
    ),
    (
        1_360,
        860,
        SmokeVisualState::Connected,
        ShellLayoutMode::Standard,
    ),
    (
        1_360,
        860,
        SmokeVisualState::Single,
        ShellLayoutMode::Standard,
    ),
    (
        1_360,
        860,
        SmokeVisualState::HSplit,
        ShellLayoutMode::Standard,
    ),
    (
        1_360,
        860,
        SmokeVisualState::VSplit,
        ShellLayoutMode::Standard,
    ),
    (
        1_360,
        860,
        SmokeVisualState::TopBottom3,
        ShellLayoutMode::Standard,
    ),
    (
        1_360,
        860,
        SmokeVisualState::Grid,
        ShellLayoutMode::Standard,
    ),
    (
        1_360,
        860,
        SmokeVisualState::Editor,
        ShellLayoutMode::Standard,
    ),
    (
        1_360,
        860,
        SmokeVisualState::Settings,
        ShellLayoutMode::Standard,
    ),
    (
        1_360,
        860,
        SmokeVisualState::Import,
        ShellLayoutMode::Standard,
    ),
    (
        1_360,
        860,
        SmokeVisualState::HostKey,
        ShellLayoutMode::Standard,
    ),
    (
        1_360,
        860,
        SmokeVisualState::Authentication,
        ShellLayoutMode::Standard,
    ),
    (
        1_360,
        860,
        SmokeVisualState::Failure,
        ShellLayoutMode::Standard,
    ),
    (
        1_360,
        860,
        SmokeVisualState::Recovery,
        ShellLayoutMode::Standard,
    ),
    (
        1_920,
        1_080,
        SmokeVisualState::Connected,
        ShellLayoutMode::Wide,
    ),
    (
        1_920,
        1_080,
        SmokeVisualState::TwentyTabs,
        ShellLayoutMode::Wide,
    ),
    (1_920, 1_080, SmokeVisualState::Grid, ShellLayoutMode::Wide),
    (
        1_920,
        1_080,
        SmokeVisualState::Editor,
        ShellLayoutMode::Wide,
    ),
    (
        1_920,
        1_080,
        SmokeVisualState::Settings,
        ShellLayoutMode::Wide,
    ),
    (
        1_920,
        1_080,
        SmokeVisualState::Import,
        ShellLayoutMode::Wide,
    ),
];

fn valid_checkpoint_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 96
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}
