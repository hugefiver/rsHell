use std::collections::{BTreeSet, HashSet};

use rshell_core::{
    CatalogMutation, ConnectionCatalog, ConnectionGroup, ConnectionId, GroupId, SecretUpdate,
    TransportKind, UiCommand,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SidebarRow {
    Group {
        id: GroupId,
        depth: usize,
        name: String,
    },
    Connection {
        id: ConnectionId,
        group_id: Option<GroupId>,
        depth: usize,
        name: String,
        metadata: String,
        tags: BTreeSet<String>,
    },
}

impl SidebarRow {
    pub fn tags(&self) -> &BTreeSet<String> {
        match self {
            Self::Connection { tags, .. } => tags,
            Self::Group { .. } => empty_tags(),
        }
    }
}

fn empty_tags() -> &'static BTreeSet<String> {
    static EMPTY: std::sync::OnceLock<BTreeSet<String>> = std::sync::OnceLock::new();
    EMPTY.get_or_init(BTreeSet::new)
}

#[derive(Debug, Clone, PartialEq)]
pub struct SidebarViewModel {
    catalog: ConnectionCatalog,
    query: String,
}

impl SidebarViewModel {
    pub fn new(catalog: ConnectionCatalog) -> Self {
        Self {
            catalog,
            query: String::new(),
        }
    }

    pub fn set_catalog(&mut self, catalog: ConnectionCatalog) {
        self.catalog = catalog;
    }

    pub fn catalog(&self) -> &ConnectionCatalog {
        &self.catalog
    }

    pub fn set_query(&mut self, query: impl Into<String>) {
        self.query = query.into();
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn rows(&self) -> Vec<SidebarRow> {
        let matched = self
            .catalog
            .search(&self.query)
            .into_iter()
            .collect::<HashSet<_>>();
        let filtering = !self.query.trim().is_empty();
        let mut rows = Vec::new();

        for connection in self.catalog.ordered_ids(None) {
            if !filtering || matched.contains(&connection) {
                self.push_connection(&mut rows, connection, 0);
            }
        }
        for group in self.groups_for(None) {
            self.push_group(&mut rows, group.id, 0, &matched, filtering);
        }
        rows
    }

    pub fn connection_ids(&self) -> Vec<ConnectionId> {
        self.rows()
            .into_iter()
            .filter_map(|row| match row {
                SidebarRow::Connection { id, .. } => Some(id),
                SidebarRow::Group { .. } => None,
            })
            .collect()
    }

    fn push_group(
        &self,
        rows: &mut Vec<SidebarRow>,
        id: GroupId,
        depth: usize,
        matched: &HashSet<ConnectionId>,
        filtering: bool,
    ) -> bool {
        let start = rows.len();
        let Some(group) = self.catalog.groups.get(&id) else {
            return false;
        };
        rows.push(SidebarRow::Group {
            id,
            depth,
            name: group.name.clone(),
        });
        let mut has_match = false;
        for connection in self.catalog.ordered_ids(Some(id)) {
            if !filtering || matched.contains(&connection) {
                self.push_connection(rows, connection, depth + 1);
                has_match = true;
            }
        }
        for child in self.groups_for(Some(id)) {
            has_match |= self.push_group(rows, child.id, depth + 1, matched, filtering);
        }
        if filtering && !has_match {
            rows.truncate(start);
        }
        !filtering || has_match
    }

    fn push_connection(&self, rows: &mut Vec<SidebarRow>, id: ConnectionId, depth: usize) {
        let Some(profile) = self.catalog.connections.get(&id) else {
            return;
        };
        let transport = match profile.transport {
            TransportKind::SystemOpenSsh => "OpenSSH",
            TransportKind::NativeSsh => "Native SSH",
        };
        let endpoint = if profile.username.is_empty() {
            format!("{}:{}", profile.host, profile.port)
        } else {
            format!("{}@{}:{}", profile.username, profile.host, profile.port)
        };
        rows.push(SidebarRow::Connection {
            id,
            group_id: profile.group_id,
            depth,
            name: profile.name.clone(),
            metadata: format!("{endpoint} · {transport}"),
            tags: profile.tags.clone(),
        });
    }

    fn groups_for(&self, parent: Option<GroupId>) -> Vec<&ConnectionGroup> {
        let mut groups = self
            .catalog
            .groups
            .values()
            .filter(|group| group.parent_id == parent)
            .collect::<Vec<_>>();
        groups.sort_by_key(|group| (group.position, group.id));
        groups
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SidebarAction {
    Search(String),
    Duplicate {
        source: ConnectionId,
        destination: Option<GroupId>,
    },
    MoveConnection {
        connection: ConnectionId,
        destination: Option<GroupId>,
        position: usize,
    },
    DeleteConnection(ConnectionId),
    CreateGroup(ConnectionGroup),
    RenameGroup {
        group: GroupId,
        name: String,
    },
    MoveGroup {
        group: GroupId,
        parent: Option<GroupId>,
        position: usize,
    },
    DeleteGroup(GroupId),
    SetTags {
        connection: ConnectionId,
        tags: BTreeSet<String>,
    },
}

impl SidebarAction {
    pub fn into_command(self) -> UiCommand {
        if let Self::Search(query) = self {
            return UiCommand::SearchConnections(query);
        }
        let mutation = match self {
            Self::Search(_) => unreachable!(),
            Self::Duplicate {
                source,
                destination,
            } => CatalogMutation::Duplicate {
                source,
                destination,
            },
            Self::MoveConnection {
                connection,
                destination,
                position,
            } => CatalogMutation::Move {
                connection,
                destination,
                position,
            },
            Self::DeleteConnection(id) => CatalogMutation::Delete(id),
            Self::CreateGroup(group) => CatalogMutation::CreateGroup(group),
            Self::RenameGroup { group, name } => CatalogMutation::RenameGroup { group, name },
            Self::MoveGroup {
                group,
                parent,
                position,
            } => CatalogMutation::MoveGroup {
                group,
                parent,
                position,
            },
            Self::DeleteGroup(group) => CatalogMutation::DeleteGroup(group),
            Self::SetTags { connection, tags } => CatalogMutation::SetTags { connection, tags },
        };
        UiCommand::ApplyCatalog {
            mutation,
            secret: SecretUpdate::Unchanged,
        }
    }
}
