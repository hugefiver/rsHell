use rshell_core::{ConnectionId, GroupId};

use crate::{SidebarAction, SidebarRow, SidebarViewModel};

pub(crate) fn selected_row(view: &SidebarViewModel, selected: Option<usize>) -> Option<SidebarRow> {
    selected.and_then(|index| view.rows().get(index).cloned())
}

pub(crate) fn index_for_identity(
    view: &SidebarViewModel,
    selected: Option<SidebarRow>,
) -> Option<usize> {
    let selected = selected?;
    view.rows().iter().position(|row| match (&selected, row) {
        (
            SidebarRow::Connection { id: selected, .. },
            SidebarRow::Connection { id: candidate, .. },
        ) => selected == candidate,
        (SidebarRow::Group { id: selected, .. }, SidebarRow::Group { id: candidate, .. }) => {
            selected == candidate
        }
        _ => false,
    })
}

pub(crate) fn selected_connection(
    view: &SidebarViewModel,
    selected: Option<usize>,
) -> Option<(ConnectionId, Option<GroupId>)> {
    match selected_row(view, selected) {
        Some(SidebarRow::Connection { id, group_id, .. }) => Some((id, group_id)),
        _ => None,
    }
}

pub(crate) fn selected_group(view: &SidebarViewModel, selected: Option<usize>) -> Option<GroupId> {
    match selected_row(view, selected) {
        Some(SidebarRow::Group { id, .. }) => Some(id),
        Some(SidebarRow::Connection { group_id, .. }) => group_id,
        None => None,
    }
}

pub(crate) fn delete_action(
    view: &SidebarViewModel,
    selected: Option<usize>,
) -> Option<SidebarAction> {
    match selected_row(view, selected) {
        Some(SidebarRow::Connection { id, .. }) => Some(SidebarAction::DeleteConnection(id)),
        Some(SidebarRow::Group { id, .. }) => Some(SidebarAction::DeleteGroup(id)),
        None => None,
    }
}
