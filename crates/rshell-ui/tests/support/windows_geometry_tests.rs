use super::*;
use std::cell::Cell;

#[test]
fn both_failure_exits_share_one_budget_and_success_never_samples() {
    let budget = AtomicBool::new(false);
    let samples = Cell::new(0);
    let sample = || {
        samples.set(samples.get() + 1);
        Snapshot::default()
    };
    assert!(failure_lines(true, FailurePhase::FrameClock, &budget, sample).is_none());
    assert_eq!(samples.get(), 0);
    let first = failure_lines(false, FailurePhase::FrameClock, &budget, sample).unwrap();
    assert_eq!(first.len(), 7);
    assert!(first[0].contains("phase=frame_clock"));
    assert!(failure_lines(false, FailurePhase::AfterPaint, &budget, sample).is_none());
    assert_eq!(samples.get(), 1);

    let reversed = AtomicBool::new(false);
    let second = failure_lines(false, FailurePhase::AfterPaint, &reversed, sample).unwrap();
    assert_eq!(second.len(), 7);
    assert!(second[0].contains("phase=after_paint"));
    assert!(failure_lines(false, FailurePhase::FrameClock, &reversed, sample).is_none());
    assert_eq!(samples.get(), 2);
}

#[test]
fn fixed_grammar_identifies_sources_units_and_unavailable_without_text_leaks() {
    let empty = Snapshot::default().lines(FailurePhase::AfterPaint);
    assert_eq!(empty.len(), 7);
    assert_eq!(
        empty[0],
        "WIN_FRAME_GEOMETRY phase=after_paint units=gtk_gdk_logical_win32_physical_or_dpi_virtualized_unknown"
    );
    assert_eq!(empty[2], "WIN_FRAME_GEOMETRY gtk_root=unavailable");
    assert_eq!(
        empty[3],
        "WIN_FRAME_GEOMETRY gdk_surface=unavailable gdk_monitor_xywh=unavailable"
    );
    assert!(empty[6].contains("win32_target_client_screen_xywh=unavailable"));
    let sampled = Snapshot {
        target: WidgetState {
            size: (798, 598),
            scale: 2,
            mapped: true,
            realized: true,
        },
        root: Some((
            WidgetState {
                size: (1022, 726),
                scale: 2,
                mapped: true,
                realized: true,
            },
            (1360, 860),
            false,
            false,
        )),
        surface: Some((1022, 726, 2)),
        gdk_monitor: Some((Rect(0, 0, 1920, 1080), 2)),
        primary: Some((1920, 1080)),
        virtual_screen: Some(Rect(-1920, 0, 3840, 1080)),
        primary_workarea: Some(Rect(0, 0, 1920, 1040)),
        monitor: Some((Rect(0, 0, 1920, 1080), Rect(0, 0, 1920, 1040))),
        window: Some(Rect(10, 20, 1040, 780)),
        client_local: Some(Rect(0, 0, 1022, 726)),
        client_screen: Some(Rect(19, 35, 1022, 726)),
    }
    .lines(FailurePhase::FrameClock);
    assert!(sampled[1].contains("gtk_target=798x598 scale=2 mapped=true realized=true"));
    assert!(sampled[2].contains("default=1360x860"));
    assert!(sampled[3].contains("gdk_monitor_xywh=0,0,1920,1080 scale=2"));
    assert!(sampled[4].contains("win32_virtual_xywh=-1920,0,3840,1080"));
    assert!(sampled[5].contains("win32_target_work_xywh=0,0,1920,1040"));
    assert!(sampled[6].contains("win32_target_client_screen_xywh=19,35,1022,726"));
    for line in sampled.iter().chain(empty.iter()) {
        assert!(line.starts_with("WIN_FRAME_GEOMETRY "));
        assert!(
            line.chars()
                .all(|c| c.is_ascii_alphanumeric() || " _=,-".contains(c))
        );
        for secret in [
            "C:\\Users\\private",
            "session-title",
            "\nINJECTED",
            "0xDEADBEEF",
        ] {
            assert!(!line.contains(secret));
        }
    }
}
