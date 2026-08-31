use rshell_ui::{ShellLayout, ShellLayoutMode};

#[test]
fn exact_widths_select_the_approved_shell_modes() {
    for width in [1, 800, 899] {
        assert_eq!(
            ShellLayout::for_width(width),
            ShellLayout {
                mode: ShellLayoutMode::Compact,
                navigation_width: 48,
                sidebar_overlay: true,
                text_global_actions: false,
                pane_actions_compact: true,
            },
            "width {width}"
        );
    }

    for width in [900, 1_360, 1_439] {
        assert_eq!(
            ShellLayout::for_width(width),
            ShellLayout {
                mode: ShellLayoutMode::Standard,
                navigation_width: 260,
                sidebar_overlay: false,
                text_global_actions: true,
                pane_actions_compact: false,
            },
            "width {width}"
        );
    }

    for width in [1_440, 1_920] {
        assert_eq!(
            ShellLayout::for_width(width),
            ShellLayout {
                mode: ShellLayoutMode::Wide,
                navigation_width: 280,
                sidebar_overlay: false,
                text_global_actions: true,
                pane_actions_compact: false,
            },
            "width {width}"
        );
    }
}

#[test]
fn width_decision_is_available_in_const_context() {
    const COMPACT: ShellLayout = ShellLayout::for_width(899);
    const STANDARD: ShellLayout = ShellLayout::for_width(900);
    const WIDE: ShellLayout = ShellLayout::for_width(1_440);

    assert_eq!(COMPACT.mode, ShellLayoutMode::Compact);
    assert_eq!(STANDARD.mode, ShellLayoutMode::Standard);
    assert_eq!(WIDE.mode, ShellLayoutMode::Wide);
}
