use crate::{REQUIRED_SMOKE_VISUAL_MATRIX, ShellLayoutMode, SmokeVisualState};

const WINDOWS_SESSION_ZERO_TAIL: [(i32, i32, SmokeVisualState, ShellLayoutMode); 6] = [
    (
        1_000,
        700,
        SmokeVisualState::Connected,
        ShellLayoutMode::Standard,
    ),
    (
        1_000,
        700,
        SmokeVisualState::TwentyTabs,
        ShellLayoutMode::Standard,
    ),
    (
        1_000,
        700,
        SmokeVisualState::Grid,
        ShellLayoutMode::Standard,
    ),
    (
        1_000,
        700,
        SmokeVisualState::Editor,
        ShellLayoutMode::Standard,
    ),
    (
        1_000,
        700,
        SmokeVisualState::Settings,
        ShellLayoutMode::Standard,
    ),
    (
        1_000,
        700,
        SmokeVisualState::Import,
        ShellLayoutMode::Standard,
    ),
];

pub fn visual_matrix() -> Vec<(i32, i32, SmokeVisualState, ShellLayoutMode)> {
    #[cfg(windows)]
    {
        let mut matrix = REQUIRED_SMOKE_VISUAL_MATRIX[..20].to_vec();
        matrix.extend(WINDOWS_SESSION_ZERO_TAIL);
        matrix
    }
    #[cfg(not(windows))]
    {
        REQUIRED_SMOKE_VISUAL_MATRIX.to_vec()
    }
}

#[cfg(test)]
mod tests {
    use crate::ShellLayoutMode;

    #[test]
    fn windows_session_zero_keeps_all_states_without_claiming_wide() {
        let matrix = super::visual_matrix();
        assert_eq!(matrix.len(), 26);
        #[cfg(windows)]
        assert!(matrix[20..].iter().all(|(width, height, _, mode)| {
            (*width, *height, *mode) == (1_000, 700, ShellLayoutMode::Standard)
        }));
    }
}
