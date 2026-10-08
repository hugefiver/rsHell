use crate::{PaneHostModel, PanePageKind};
use rshell_core::{NewLocalTabIdentity, PaneId, SessionId, SessionState, TabId};
use std::collections::BTreeMap;

pub(super) type Identity = NewLocalTabIdentity;

pub(super) fn selected(model: &PaneHostModel) -> Option<(Identity, PanePageKind)> {
    let tab = model.active_tab()?;
    let pane = model.active_pane(tab)?;
    let view = model.pane(pane)?;
    if view.state() == SessionState::Closing {
        return None;
    }
    Some((
        Identity {
            tab,
            pane,
            session: view.session()?,
        },
        view.page(),
    ))
}

enum Target {
    Receipt {
        identity: Option<Identity>,
        seen: bool,
    },
    Activation(Identity),
}

#[derive(Default)]
pub(super) struct FocusPolicy {
    pub generation: u64,
    target: Option<Target>,
    available: BTreeMap<PaneId, (TabId, Option<SessionId>, PanePageKind)>,
    selected: Option<(Identity, PanePageKind)>,
}

impl FocusPolicy {
    pub fn cancel(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.target = None;
    }
    pub fn begin_new(&mut self) -> u64 {
        self.cancel();
        self.target = Some(Target::Receipt {
            identity: None,
            seen: false,
        });
        self.generation
    }
    pub fn created(&mut self, generation: u64, identity: Identity) {
        if self.generation != generation
            || !matches!(self.target, Some(Target::Receipt { identity: None, .. }))
        {
            return;
        }
        self.target = Some(Target::Receipt {
            identity: Some(identity),
            seen: false,
        });
        self.validate();
    }
    pub fn activate(&mut self, identity: Identity) {
        self.cancel();
        self.target = Some(Target::Activation(identity));
    }
    pub fn active(&self) -> bool {
        self.target.is_some()
    }
    pub fn observed(&self) -> bool {
        matches!(
            self.target,
            Some(Target::Activation(_)) | Some(Target::Receipt { seen: true, .. })
        )
    }
    pub fn bound(&self) -> Option<Identity> {
        match self.target {
            Some(Target::Activation(id))
            | Some(Target::Receipt {
                identity: Some(id), ..
            }) => Some(id),
            _ => None,
        }
    }
    pub fn synchronize(&mut self, model: &PaneHostModel) {
        self.selected = selected(model);
        self.available = model
            .view_model()
            .workspace
            .tabs
            .iter()
            .flat_map(|tab| {
                tab.pane_tree
                    .pane_ids()
                    .into_iter()
                    .filter_map(move |pane| {
                        model.pane(pane).map(|view| {
                            let page = if view.state() == SessionState::Closing {
                                PanePageKind::Status
                            } else {
                                view.page()
                            };
                            (pane, (tab.id, view.session(), page))
                        })
                    })
            })
            .collect();
        self.validate();
    }
    fn validate(&mut self) {
        let Some(expected) = self.bound() else {
            return;
        };
        if let Some((tab, session, page)) = self.available.get(&expected.pane) {
            if *tab != expected.tab
                || *session != Some(expected.session)
                || !matches!(page, PanePageKind::Pending | PanePageKind::Terminal)
                || self.selected.is_none_or(|(id, _)| id != expected)
            {
                self.cancel();
                return;
            }
            if let Some(Target::Receipt { seen, .. }) = self.target.as_mut() {
                *seen = true;
            }
        } else if self.observed() {
            self.cancel();
        }
    }
}

#[cfg(test)]
#[path = "pane_host_focus_policy_tests.rs"]
mod tests;
