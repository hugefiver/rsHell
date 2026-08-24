use rshell_core::InteractionId;

use crate::{SmokeAction, smoke_driver_observation::SmokeObservation};

pub(crate) fn action_route_ready(
    action: &SmokeAction,
    observed: &SmokeObservation,
    last_submitted_interaction: Option<InteractionId>,
) -> bool {
    match action {
        SmokeAction::WaitWindowRealized | SmokeAction::WaitFrameContains(_) => false,
        SmokeAction::RespondHostKey { .. } | SmokeAction::RespondAuth { .. } => observed
            .active_interaction
            .is_some_and(|interaction| Some(interaction) != last_submitted_interaction),
        _ => true,
    }
}
