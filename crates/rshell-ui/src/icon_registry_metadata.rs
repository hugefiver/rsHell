use crate::ProductIcon;

#[derive(Debug, Clone, Copy)]
pub struct IconMetadata {
    pub accessible_label: &'static str,
    pub tooltip: &'static str,
    pub svg: &'static [u8],
}

impl ProductIcon {
    pub fn metadata(self) -> IconMetadata {
        let (accessible_label, tooltip, svg) = match self {
            Self::Import => (
                "Import",
                "Import connections",
                include_bytes!("../../../resources/icons/import.svg").as_slice(),
            ),
            Self::Settings => (
                "Settings",
                "Terminal settings",
                include_bytes!("../../../resources/icons/settings.svg").as_slice(),
            ),
            Self::AddConnection => (
                "Add connection",
                "Add connection",
                include_bytes!("../../../resources/icons/add-connection.svg").as_slice(),
            ),
            Self::AddGroup => (
                "Add group",
                "Add group",
                include_bytes!("../../../resources/icons/add-group.svg").as_slice(),
            ),
            Self::Edit => (
                "Edit",
                "Edit selected connection",
                include_bytes!("../../../resources/icons/edit.svg").as_slice(),
            ),
            Self::Duplicate => (
                "Duplicate",
                "Duplicate selected connection",
                include_bytes!("../../../resources/icons/duplicate.svg").as_slice(),
            ),
            Self::Delete => (
                "Delete",
                "Delete selected connection",
                include_bytes!("../../../resources/icons/delete.svg").as_slice(),
            ),
            Self::CloseTab => (
                "Close",
                "Close tab or pane",
                include_bytes!("../../../resources/icons/close-tab.svg").as_slice(),
            ),
            Self::NewTab => (
                "New tab",
                "New local terminal tab",
                include_bytes!("../../../resources/icons/new-tab.svg").as_slice(),
            ),
            Self::SplitHorizontal => (
                "Split horizontally",
                "Split pane horizontally",
                include_bytes!("../../../resources/icons/split-horizontal.svg").as_slice(),
            ),
            Self::SplitVertical => (
                "Split vertically",
                "Split pane vertically",
                include_bytes!("../../../resources/icons/split-vertical.svg").as_slice(),
            ),
            Self::Retry => (
                "Reconnect",
                "Reconnect session",
                include_bytes!("../../../resources/icons/retry.svg").as_slice(),
            ),
            Self::CopyDiagnostics => (
                "Copy diagnostics",
                "Copy diagnostics",
                include_bytes!("../../../resources/icons/copy-diagnostics.svg").as_slice(),
            ),
            Self::Warning => (
                "Warning",
                "Warning",
                include_bytes!("../../../resources/icons/warning.svg").as_slice(),
            ),
            Self::SecretPresent => (
                "Secret present",
                "Secret present",
                include_bytes!("../../../resources/icons/secret-present.svg").as_slice(),
            ),
            Self::HostTrust => (
                "Host trust",
                "Host trust",
                include_bytes!("../../../resources/icons/host-trust.svg").as_slice(),
            ),
            Self::More => (
                "More",
                "Show more actions",
                include_bytes!("../../../resources/icons/more.svg").as_slice(),
            ),
            Self::Navigation => (
                "Navigation",
                "Open navigation",
                include_bytes!("../../../resources/icons/navigation.svg").as_slice(),
            ),
        };
        IconMetadata {
            accessible_label,
            tooltip,
            svg,
        }
    }
}
