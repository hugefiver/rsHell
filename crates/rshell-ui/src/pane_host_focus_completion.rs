use super::{PaneFocus, Request, Restore};
use relm4::gtk;
use rshell_core::{NewLocalTabCompletion, NewLocalTabReceipt};

pub(crate) struct NewLocalFocus {
    focus: PaneFocus,
    generation: u64,
}

impl Drop for Request {
    fn drop(&mut self) {
        if let Some(task) = self.receipt.take() {
            task.abort();
        }
    }
}

impl PaneFocus {
    pub(crate) fn begin(&self, eligible: bool) -> NewLocalFocus {
        let generation = self.state.borrow_mut().policy.begin_new();
        self.clear_watch();
        if eligible {
            self.start(true, Restore::Canvas);
        } else {
            self.invalidate();
        }
        NewLocalFocus {
            focus: self.clone(),
            generation,
        }
    }
}

impl NewLocalFocus {
    pub(crate) fn cancel(self) {
        if self.focus.state.borrow().policy.generation == self.generation {
            self.focus.invalidate();
        }
    }
    pub(crate) fn receive(self, receipt: NewLocalTabReceipt) {
        let eligible = {
            let state = self.focus.state.borrow();
            state.policy.generation == self.generation && state.request.is_some()
        };
        if !eligible {
            return;
        }
        let state = std::rc::Rc::downgrade(&self.focus.state);
        let content = self.focus.content.clone();
        let generation = self.generation;
        let task = gtk::glib::MainContext::default().spawn_local(async move {
            let outcome = receipt.await;
            let Some(state) = state.upgrade() else {
                return;
            };
            let focus = PaneFocus { state, content };
            let own_task = {
                let mut state = focus.state.borrow_mut();
                if state.policy.generation != generation {
                    return;
                }
                state.request.as_mut().and_then(|r| r.receipt.take())
            };
            drop(own_task); // Detach completed task; never abort itself while borrowing state.
            match outcome {
                Ok(NewLocalTabCompletion::Created(identity)) => {
                    focus
                        .state
                        .borrow_mut()
                        .policy
                        .created(generation, identity);
                    if !focus.state.borrow().policy.active() {
                        focus.clear_watch();
                        return;
                    }
                    focus.watch_readiness();
                    focus.try_focus(generation);
                    focus.schedule_layout(generation);
                }
                Ok(NewLocalTabCompletion::NoCreation) | Err(_) => focus.invalidate(),
            }
        });
        let mut task = Some(task);
        {
            let mut state = self.focus.state.borrow_mut();
            if state.policy.generation == generation
                && let Some(request) = state.request.as_mut()
            {
                request.receipt = task.take();
            }
        }
        if let Some(task) = task {
            task.abort();
        }
    }
}
