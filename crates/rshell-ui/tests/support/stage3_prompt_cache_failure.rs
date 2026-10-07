use std::{
    any::Any,
    io::Write,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind, set_hook, take_hook},
    sync::{Arc, Mutex},
    thread,
};

// Only this fixture's post-verdict attempts share the scoped secondary hook.
static SECONDARY_HOOK: Mutex<()> = Mutex::new(());

fn secondary_failed(result: thread::Result<()>) -> bool {
    if let Err(payload) = result {
        // A secondary payload's arbitrary Drop must not mask the primary either.
        std::mem::forget(payload);
        true
    } else {
        false
    }
}

pub(super) fn resume_after_failure(
    primary: Box<dyn Any + Send>,
    report: impl FnOnce(),
    close: impl FnOnce(),
) -> ! {
    // The original unwind is already caught, outside any GTK FFI callback.
    let guard = SECONDARY_HOOK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let previous = Arc::new(take_hook());
    let forward = previous.clone();
    let owner = thread::current().id();
    set_hook(Box::new(move |info| {
        if thread::current().id() != owner {
            forward(info);
        }
    }));
    let report_failed = secondary_failed(catch_unwind(AssertUnwindSafe(report)));
    let close_failed = secondary_failed(catch_unwind(AssertUnwindSafe(close)));
    set_hook(Box::new(move |info| previous(info)));
    drop(guard);
    if report_failed {
        let _ = writeln!(
            std::io::stderr(),
            "PROMPT_CACHE_NATIVE secondary_failure phase=report failed=true"
        );
    }
    if close_failed {
        let _ = writeln!(
            std::io::stderr(),
            "PROMPT_CACHE_NATIVE secondary_failure phase=close failed=true"
        );
    }
    resume_unwind(primary);
}

#[cfg(test)]
#[path = "stage3_prompt_cache_failure_tests.rs"]
mod tests;
