use std::collections::BTreeSet;

use crate::{ConnectionEditorViewModel, EditorTextField};

pub(crate) fn set_text(
    view: &mut ConnectionEditorViewModel,
    field: EditorTextField,
    value: String,
) {
    match field {
        EditorTextField::Name => view.name = value,
        EditorTextField::Host => view.host = value,
        EditorTextField::Username => view.username = value,
        EditorTextField::IdentityFile => view.identity_file = value,
        EditorTextField::RemoteCommand => view.remote_command = value,
        EditorTextField::Note => view.note = value,
        EditorTextField::Tags => view.tags = split_tags(&value),
    }
}

fn split_tags(value: &str) -> BTreeSet<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(str::to_owned)
        .collect()
}
