use super::resume_after_failure;
use std::{
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind, panic_any},
    sync::Arc,
};

struct ReportFailure;
struct CloseFailure;

fn verify_primary(report_fails: bool, close_fails: bool) {
    let primary = Arc::new(());
    let retained = Box::new(primary.clone());
    let payload_address = std::ptr::from_ref(retained.as_ref());
    let attempts = RefCell::new(Vec::new());
    let result: Result<(), _> = catch_unwind(AssertUnwindSafe(|| {
        resume_after_failure(
            retained,
            || {
                attempts.borrow_mut().push("report");
                if report_fails {
                    panic_any(ReportFailure);
                }
            },
            || {
                attempts.borrow_mut().push("close");
                if close_fails {
                    panic_any(CloseFailure);
                }
            },
        );
    }));
    assert_eq!(*attempts.borrow(), ["report", "close"]);
    let resumed = result.expect_err("original failure must always be resumed");
    let resumed = resumed
        .downcast_ref::<Arc<()>>()
        .expect("secondary failure replaced the original payload");
    assert!(Arc::ptr_eq(resumed, &primary));
    assert!(std::ptr::eq(resumed, payload_address));
}

#[test]
fn report_failure_still_attempts_close_and_preserves_primary() {
    verify_primary(true, false);
}

#[test]
fn both_secondary_failures_still_attempt_close_and_preserve_primary() {
    verify_primary(true, true);
}

#[test]
fn close_failure_preserves_primary() {
    verify_primary(false, true);
}

#[test]
fn successful_secondary_attempts_preserve_primary() {
    verify_primary(false, false);
}
