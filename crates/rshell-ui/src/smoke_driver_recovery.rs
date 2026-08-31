use crate::SmokeInterruptEvidence;

pub(crate) fn interrupt_complete(
    before: Option<SmokeInterruptEvidence>,
    now: SmokeInterruptEvidence,
) -> bool {
    now.sequence > before.map_or(0, |evidence| evidence.sequence)
        && now.command_count == 1
        && now.wire_byte == 0x03
        && now.exact_etx
        && now.enhanced_encoder_bypassed
        && now.surviving_tui
        && now.notice_visible
        && now.reset_generation.is_none()
        && !now.modes_clean
        && now.replacement_character_count == 0
        && now.old_tui_overlap
}

pub(crate) fn reset_complete(
    before: Option<SmokeInterruptEvidence>,
    now: SmokeInterruptEvidence,
) -> bool {
    before.is_some_and(|evidence| {
        evidence.surviving_tui
            && evidence.notice_visible
            && evidence.reset_generation.is_none()
            && !evidence.modes_clean
    }) && now.sequence > before.map_or(0, |evidence| evidence.sequence)
        && now.command_count == 1
        && now.wire_byte == 0x03
        && now.exact_etx
        && now.enhanced_encoder_bypassed
        && now.surviving_tui
        && !now.notice_visible
        && now.reset_generation.is_some()
        && now.modes_clean
        && now.replacement_character_count == 0
        && !now.old_tui_overlap
}

#[cfg(test)]
mod tests {
    use super::{interrupt_complete, reset_complete};
    use crate::SmokeInterruptEvidence;

    fn interrupted() -> SmokeInterruptEvidence {
        SmokeInterruptEvidence {
            sequence: 7,
            command_count: 1,
            wire_byte: 0x03,
            exact_etx: true,
            enhanced_encoder_bypassed: true,
            surviving_tui: true,
            notice_visible: true,
            reset_generation: None,
            modes_clean: false,
            replacement_character_count: 0,
            old_tui_overlap: true,
        }
    }

    #[test]
    fn interrupt_requires_one_exact_etx_and_preserves_surviving_tui() {
        let before = SmokeInterruptEvidence {
            sequence: 6,
            ..interrupted()
        };
        assert!(interrupt_complete(Some(before), interrupted()));
        for invalid in [
            SmokeInterruptEvidence {
                command_count: 2,
                ..interrupted()
            },
            SmokeInterruptEvidence {
                wire_byte: 0x1b,
                ..interrupted()
            },
            SmokeInterruptEvidence {
                modes_clean: true,
                notice_visible: false,
                old_tui_overlap: false,
                ..interrupted()
            },
        ] {
            assert!(!interrupt_complete(Some(before), invalid));
        }
    }

    #[test]
    fn reset_requires_explicit_new_clean_frame_without_overlap() {
        let after = SmokeInterruptEvidence {
            sequence: 8,
            notice_visible: false,
            reset_generation: Some(42),
            modes_clean: true,
            old_tui_overlap: false,
            ..interrupted()
        };
        assert!(reset_complete(Some(interrupted()), after));
        assert!(!reset_complete(
            Some(SmokeInterruptEvidence {
                modes_clean: true,
                ..interrupted()
            }),
            after
        ));
        assert!(!reset_complete(
            Some(interrupted()),
            SmokeInterruptEvidence {
                replacement_character_count: 1,
                ..after
            }
        ));
    }
}
