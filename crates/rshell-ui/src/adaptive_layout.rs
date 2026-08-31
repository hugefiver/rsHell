#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellLayoutMode {
    Compact,
    Standard,
    Wide,
}

impl ShellLayoutMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Standard => "standard",
            Self::Wide => "wide",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "compact" => Some(Self::Compact),
            "standard" => Some(Self::Standard),
            "wide" => Some(Self::Wide),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellLayout {
    pub mode: ShellLayoutMode,
    pub navigation_width: i32,
    pub sidebar_overlay: bool,
    pub text_global_actions: bool,
    pub pane_actions_compact: bool,
}

impl ShellLayout {
    pub const fn for_width(width: i32) -> Self {
        if width < 900 {
            Self {
                mode: ShellLayoutMode::Compact,
                navigation_width: 48,
                sidebar_overlay: true,
                text_global_actions: false,
                pane_actions_compact: true,
            }
        } else if width < 1_440 {
            Self {
                mode: ShellLayoutMode::Standard,
                navigation_width: 260,
                sidebar_overlay: false,
                text_global_actions: true,
                pane_actions_compact: false,
            }
        } else {
            Self {
                mode: ShellLayoutMode::Wide,
                navigation_width: 280,
                sidebar_overlay: false,
                text_global_actions: true,
                pane_actions_compact: false,
            }
        }
    }
}
