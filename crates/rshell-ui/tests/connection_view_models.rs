use std::{collections::BTreeSet, path::PathBuf};

use rshell_core::{
    AuthenticationKind, CatalogMutation, ConnectionCatalog, ConnectionGroup, ConnectionProfile,
    SecretUpdate, TerminalProfileId, TransportKind, UiCommand,
};
use rshell_ui::{
    AuthenticationCapabilities, ConnectionEditorDraft, ConnectionEditorMsg, EditorValidationError,
    SecretEditKind, SidebarAction, SidebarRow, SidebarViewModel,
};

fn profile(name: &str, host: &str) -> ConnectionProfile {
    let mut profile = ConnectionProfile::new(name, host);
    profile.username = "operator".into();
    profile
}

fn existing_password_profile() -> ConnectionProfile {
    let mut profile = profile("Production", "prod.example.test");
    profile.transport = TransportKind::NativeSsh;
    profile.authentication = AuthenticationKind::Password;
    profile.credential_ref = Some("rshell://credential/existing".into());
    profile
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SecretOutcome {
    Unchanged,
    Set,
    Clear,
    Required,
}

#[derive(Clone, Copy)]
enum SecretInput {
    Untouched,
    Empty,
    Value,
}

struct SecretPolicyCase {
    name: &'static str,
    original_transport: TransportKind,
    original_authentication: AuthenticationKind,
    had_credential: bool,
    resulting_transport: TransportKind,
    resulting_authentication: AuthenticationKind,
    input: SecretInput,
    expected: SecretOutcome,
}

#[derive(Clone, Copy)]
enum DraftOrigin {
    New,
    Existing { had_credential: bool },
}

#[test]
fn editor_validates_before_send_and_preserves_secret_semantics() {
    let mut editor = ConnectionEditorDraft::edit(&existing_password_profile());

    assert_eq!(editor.secret_kind(), SecretEditKind::Untouched);
    let command = editor.save_command().expect("untouched profile is valid");
    assert!(matches!(
        command.secret_update(),
        Some(SecretUpdate::Unchanged)
    ));

    editor.mark_secret_edited();
    let command = editor
        .save_command()
        .expect("explicit clear is represented");
    assert!(matches!(command.secret_update(), Some(SecretUpdate::Clear)));

    editor.view_mut().port = "0".into();
    assert_eq!(
        editor.save_command().unwrap_err(),
        EditorValidationError::InvalidPort
    );
    editor.view_mut().port = "65536".into();
    assert_eq!(
        editor.save_command().unwrap_err(),
        EditorValidationError::InvalidPort
    );
    editor.view_mut().port = "22.5".into();
    assert_eq!(
        editor.save_command().unwrap_err(),
        EditorValidationError::InvalidPort
    );
}

#[test]
fn new_password_profile_requires_an_explicit_nonempty_secret() {
    let mut editor = ConnectionEditorDraft::create(None);
    editor.view_mut().name = "New host".into();
    editor.view_mut().host = "new.example.test".into();
    editor.view_mut().transport = TransportKind::NativeSsh;
    editor.view_mut().authentication = AuthenticationKind::Password;

    assert_eq!(
        editor.save_command().unwrap_err(),
        EditorValidationError::SecretRequired
    );

    editor.set_secret("task15-new-secret");
    let command = editor.save_command().expect("new password secret is set");
    assert!(matches!(
        command.secret_update(),
        Some(SecretUpdate::Set(_))
    ));
    assert!(editor.secret_is_empty(), "secret must be moved only once");
}

#[test]
fn existing_profile_secret_policy_never_reinterprets_or_retains_stale_credentials() {
    const SENTINEL: &str = "TASK15-TRANSITION-SENTINEL";
    let cases = [
        SecretPolicyCase {
            name: "same password credential",
            original_transport: TransportKind::NativeSsh,
            original_authentication: AuthenticationKind::Password,
            had_credential: true,
            resulting_transport: TransportKind::NativeSsh,
            resulting_authentication: AuthenticationKind::Password,
            input: SecretInput::Untouched,
            expected: SecretOutcome::Unchanged,
        },
        SecretPolicyCase {
            name: "same public-key passphrase",
            original_transport: TransportKind::NativeSsh,
            original_authentication: AuthenticationKind::PublicKey,
            had_credential: true,
            resulting_transport: TransportKind::NativeSsh,
            resulting_authentication: AuthenticationKind::PublicKey,
            input: SecretInput::Untouched,
            expected: SecretOutcome::Unchanged,
        },
        SecretPolicyCase {
            name: "agent to password requires replacement",
            original_transport: TransportKind::SystemOpenSsh,
            original_authentication: AuthenticationKind::Agent,
            had_credential: false,
            resulting_transport: TransportKind::NativeSsh,
            resulting_authentication: AuthenticationKind::Password,
            input: SecretInput::Untouched,
            expected: SecretOutcome::Required,
        },
        SecretPolicyCase {
            name: "password without ref still requires replacement",
            original_transport: TransportKind::NativeSsh,
            original_authentication: AuthenticationKind::Password,
            had_credential: false,
            resulting_transport: TransportKind::NativeSsh,
            resulting_authentication: AuthenticationKind::Password,
            input: SecretInput::Empty,
            expected: SecretOutcome::Required,
        },
        SecretPolicyCase {
            name: "public-key passphrase cannot become a password",
            original_transport: TransportKind::NativeSsh,
            original_authentication: AuthenticationKind::PublicKey,
            had_credential: true,
            resulting_transport: TransportKind::NativeSsh,
            resulting_authentication: AuthenticationKind::Password,
            input: SecretInput::Untouched,
            expected: SecretOutcome::Required,
        },
        SecretPolicyCase {
            name: "different auth supplies password once",
            original_transport: TransportKind::NativeSsh,
            original_authentication: AuthenticationKind::KeyboardInteractive,
            had_credential: false,
            resulting_transport: TransportKind::NativeSsh,
            resulting_authentication: AuthenticationKind::Password,
            input: SecretInput::Value,
            expected: SecretOutcome::Set,
        },
        SecretPolicyCase {
            name: "password ref is cleared before public-key auth",
            original_transport: TransportKind::NativeSsh,
            original_authentication: AuthenticationKind::Password,
            had_credential: true,
            resulting_transport: TransportKind::NativeSsh,
            resulting_authentication: AuthenticationKind::PublicKey,
            input: SecretInput::Untouched,
            expected: SecretOutcome::Clear,
        },
        SecretPolicyCase {
            name: "public-key transition without stale ref stays unchanged",
            original_transport: TransportKind::NativeSsh,
            original_authentication: AuthenticationKind::KeyboardInteractive,
            had_credential: false,
            resulting_transport: TransportKind::NativeSsh,
            resulting_authentication: AuthenticationKind::PublicKey,
            input: SecretInput::Untouched,
            expected: SecretOutcome::Unchanged,
        },
        SecretPolicyCase {
            name: "edited public-key passphrase replaces stale credential",
            original_transport: TransportKind::NativeSsh,
            original_authentication: AuthenticationKind::Password,
            had_credential: true,
            resulting_transport: TransportKind::NativeSsh,
            resulting_authentication: AuthenticationKind::PublicKey,
            input: SecretInput::Value,
            expected: SecretOutcome::Set,
        },
        SecretPolicyCase {
            name: "edited empty public-key passphrase clears the ref",
            original_transport: TransportKind::NativeSsh,
            original_authentication: AuthenticationKind::PublicKey,
            had_credential: true,
            resulting_transport: TransportKind::NativeSsh,
            resulting_authentication: AuthenticationKind::PublicKey,
            input: SecretInput::Empty,
            expected: SecretOutcome::Clear,
        },
        SecretPolicyCase {
            name: "system auth clears stored managed secret",
            original_transport: TransportKind::NativeSsh,
            original_authentication: AuthenticationKind::Password,
            had_credential: true,
            resulting_transport: TransportKind::SystemOpenSsh,
            resulting_authentication: AuthenticationKind::Agent,
            input: SecretInput::Untouched,
            expected: SecretOutcome::Clear,
        },
        SecretPolicyCase {
            name: "keyboard-interactive discards edited secret and clears old ref",
            original_transport: TransportKind::NativeSsh,
            original_authentication: AuthenticationKind::Password,
            had_credential: true,
            resulting_transport: TransportKind::NativeSsh,
            resulting_authentication: AuthenticationKind::KeyboardInteractive,
            input: SecretInput::Value,
            expected: SecretOutcome::Clear,
        },
        SecretPolicyCase {
            name: "non-managed target discards edited secret without creating a ref",
            original_transport: TransportKind::SystemOpenSsh,
            original_authentication: AuthenticationKind::Agent,
            had_credential: false,
            resulting_transport: TransportKind::SystemOpenSsh,
            resulting_authentication: AuthenticationKind::Agent,
            input: SecretInput::Value,
            expected: SecretOutcome::Unchanged,
        },
        SecretPolicyCase {
            name: "non-managed target ignores edited empty without an old ref",
            original_transport: TransportKind::NativeSsh,
            original_authentication: AuthenticationKind::KeyboardInteractive,
            had_credential: false,
            resulting_transport: TransportKind::NativeSsh,
            resulting_authentication: AuthenticationKind::KeyboardInteractive,
            input: SecretInput::Empty,
            expected: SecretOutcome::Unchanged,
        },
    ];

    let mut mismatches = Vec::new();
    for case in cases {
        let mut source = profile(case.name, "policy.example.test");
        source.transport = case.original_transport;
        source.authentication = case.original_authentication;
        source.credential_ref = case.had_credential.then(|| "rshell://old".into());
        let mut editor = ConnectionEditorDraft::edit(&source);
        editor.view_mut().transport = case.resulting_transport;
        editor.view_mut().authentication = case.resulting_authentication;
        if case.resulting_authentication == AuthenticationKind::PublicKey {
            editor.view_mut().identity_file = "keys/id_ed25519".into();
        }
        match case.input {
            SecretInput::Untouched => {}
            SecretInput::Empty => editor.mark_secret_edited(),
            SecretInput::Value => editor.set_secret(SENTINEL),
        }

        assert!(!format!("{editor:?}").contains(SENTINEL));
        let result = editor.save_command();
        let observed = match result {
            Err(EditorValidationError::SecretRequired) => SecretOutcome::Required,
            Err(error) => panic!("{} failed unexpectedly: {error}", case.name),
            Ok(command) => {
                assert!(!format!("{command:?}").contains(SENTINEL));
                let UiCommand::ApplyCatalog {
                    mutation: CatalogMutation::Update(profile),
                    secret,
                } = command
                else {
                    panic!("{} did not produce a coordinator update", case.name);
                };
                assert_eq!(profile.transport, case.resulting_transport);
                assert_eq!(profile.authentication, case.resulting_authentication);
                match secret {
                    SecretUpdate::Unchanged => SecretOutcome::Unchanged,
                    SecretUpdate::Set(_) => SecretOutcome::Set,
                    SecretUpdate::Clear => SecretOutcome::Clear,
                }
            }
        };
        if observed != case.expected {
            mismatches.push((case.name, case.expected, observed));
        }
    }
    assert!(
        mismatches.is_empty(),
        "secret policy mismatches: {mismatches:#?}"
    );
}

#[test]
fn public_key_empty_secret_clears_only_an_existing_passphrase() {
    const SENTINEL: &str = "TASK15-PUBLIC-KEY-SENTINEL";
    let cases = [
        (
            "new public key empty",
            DraftOrigin::New,
            SecretInput::Empty,
            SecretOutcome::Unchanged,
        ),
        (
            "existing public key without passphrase empty",
            DraftOrigin::Existing {
                had_credential: false,
            },
            SecretInput::Empty,
            SecretOutcome::Unchanged,
        ),
        (
            "existing public key with passphrase empty",
            DraftOrigin::Existing {
                had_credential: true,
            },
            SecretInput::Empty,
            SecretOutcome::Clear,
        ),
        (
            "new public key value",
            DraftOrigin::New,
            SecretInput::Value,
            SecretOutcome::Set,
        ),
        (
            "existing public key untouched",
            DraftOrigin::Existing {
                had_credential: true,
            },
            SecretInput::Untouched,
            SecretOutcome::Unchanged,
        ),
    ];

    let mut mismatches = Vec::new();
    for (name, origin, input, expected) in cases {
        let mut editor = match origin {
            DraftOrigin::New => ConnectionEditorDraft::create(None),
            DraftOrigin::Existing { had_credential } => {
                let mut source = profile(name, "public-key.example.test");
                source.transport = TransportKind::NativeSsh;
                source.authentication = AuthenticationKind::PublicKey;
                source.identity_file = Some(PathBuf::from("keys/id_ed25519"));
                source.credential_ref = had_credential.then(|| "rshell://passphrase".into());
                ConnectionEditorDraft::edit(&source)
            }
        };
        editor.view_mut().name = name.into();
        editor.view_mut().host = "public-key.example.test".into();
        editor.view_mut().transport = TransportKind::NativeSsh;
        editor.view_mut().authentication = AuthenticationKind::PublicKey;
        editor.view_mut().identity_file = "keys/id_ed25519".into();
        match input {
            SecretInput::Untouched => {}
            SecretInput::Empty => editor.mark_secret_edited(),
            SecretInput::Value => editor.set_secret(SENTINEL),
        }

        let command = editor.save_command().expect("public-key draft is valid");
        assert!(!format!("{command:?}").contains(SENTINEL));
        let UiCommand::ApplyCatalog { mutation, secret } = command else {
            panic!("{name} did not emit ApplyCatalog");
        };
        assert_eq!(
            matches!(mutation, CatalogMutation::Create(_)),
            matches!(origin, DraftOrigin::New),
            "{name} emitted the wrong catalog mutation"
        );
        let observed = match secret {
            SecretUpdate::Unchanged => SecretOutcome::Unchanged,
            SecretUpdate::Set(_) => SecretOutcome::Set,
            SecretUpdate::Clear => SecretOutcome::Clear,
        };
        if observed != expected {
            mismatches.push((name, expected, observed));
        }
    }
    assert!(
        mismatches.is_empty(),
        "public-key empty-secret mismatches: {mismatches:#?}"
    );
}

#[test]
fn capability_matrix_matches_the_locked_core_transport_rules() {
    assert_eq!(
        AuthenticationCapabilities::for_transport(TransportKind::SystemOpenSsh).supported(),
        &[AuthenticationKind::Agent, AuthenticationKind::PublicKey]
    );
    assert_eq!(
        AuthenticationCapabilities::for_transport(TransportKind::NativeSsh).supported(),
        &[
            AuthenticationKind::Password,
            AuthenticationKind::PublicKey,
            AuthenticationKind::KeyboardInteractive,
        ]
    );

    let mut editor = ConnectionEditorDraft::create(None);
    editor.view_mut().name = "Unsupported".into();
    editor.view_mut().host = "host.example.test".into();
    editor.view_mut().transport = TransportKind::SystemOpenSsh;
    editor.view_mut().authentication = AuthenticationKind::Password;
    editor.set_secret("must-not-send");
    assert_eq!(
        editor.save_command().unwrap_err(),
        EditorValidationError::UnsupportedAuthentication
    );
}

#[test]
fn editor_projects_every_task15_profile_field_without_loading_a_secret() {
    let mut source = profile("Detailed", "detail.example.test");
    source.port = 2202;
    source.username = "admin".into();
    source.transport = TransportKind::NativeSsh;
    source.authentication = AuthenticationKind::PublicKey;
    source.identity_file = Some(PathBuf::from("keys/id_ed25519"));
    source.remote_command = Some("tmux attach".into());
    source.note = "Primary operations host".into();
    source.tags = BTreeSet::from(["linux".into(), "production".into()]);
    source.terminal_profile_id = Some(TerminalProfileId::new());
    source.terminal_overrides.font_family = Some("Cascadia Mono".into());
    source.credential_ref = Some("rshell://credential/must-not-project".into());

    let mut editor = ConnectionEditorDraft::edit(&source);
    let view = editor.view();
    assert_eq!(view.name, source.name);
    assert_eq!(view.host, source.host);
    assert_eq!(view.port, "2202");
    assert_eq!(view.username, source.username);
    assert_eq!(view.transport, source.transport);
    assert_eq!(view.authentication, source.authentication);
    assert_eq!(view.identity_file, "keys/id_ed25519");
    assert_eq!(view.remote_command, "tmux attach");
    assert_eq!(view.note, source.note);
    assert_eq!(view.tags, source.tags);
    assert_eq!(view.terminal_profile_id, source.terminal_profile_id);
    assert_eq!(
        view.terminal_overrides.font_family.as_deref(),
        Some("Cascadia Mono")
    );
    assert!(editor.secret_is_empty());

    let command = editor.save_command().expect("projected profile is valid");
    let UiCommand::ApplyCatalog { mutation, .. } = command else {
        panic!("editor must emit ApplyCatalog");
    };
    let CatalogMutation::Update(projected) = mutation else {
        panic!("editing must emit Update");
    };
    assert!(
        projected.credential_ref.is_none(),
        "credential references are restored by core for Unchanged"
    );
}

#[test]
fn secret_sentinel_never_appears_in_public_debug_or_validation_errors() {
    const SENTINEL: &str = "TASK15-SENTINEL-DO-NOT-LEAK";
    let mut editor = ConnectionEditorDraft::edit(&existing_password_profile());
    editor.set_secret(SENTINEL);

    assert!(!format!("{editor:?}").contains(SENTINEL));
    assert!(!format!("{:?}", editor.view()).contains(SENTINEL));
    assert!(
        !format!("{:?}", ConnectionEditorMsg::SecretChanged(SENTINEL.into())).contains(SENTINEL)
    );

    let command = editor.save_command().expect("valid edited secret");
    assert!(!format!("{command:?}").contains(SENTINEL));

    editor.set_secret(SENTINEL);
    editor.view_mut().host.clear();
    let error = editor.save_command().unwrap_err();
    assert!(!format!("{error:?}").contains(SENTINEL));
    assert!(!error.to_string().contains(SENTINEL));

    editor.close();
    assert!(editor.secret_is_empty());
    assert_eq!(editor.secret_kind(), SecretEditKind::Untouched);
}

#[test]
fn sidebar_renders_stable_group_tree_tags_and_unicode_search() {
    let root_group = ConnectionGroup::new("Operations");
    let mut child_group = ConnectionGroup::new("Europe");
    child_group.parent_id = Some(root_group.id);

    let mut ungrouped = profile("Ångström Lab", "lab.example.test");
    ungrouped.position = 0;
    ungrouped.tags = BTreeSet::from(["研究".into()]);
    let mut grouped = profile("Zürich", "zrh.example.test");
    grouped.group_id = Some(child_group.id);
    grouped.position = 0;
    grouped.tags = BTreeSet::from(["运维".into(), "production".into()]);

    let mut catalog = ConnectionCatalog::default();
    catalog.groups.insert(root_group.id, root_group.clone());
    catalog.groups.insert(child_group.id, child_group.clone());
    catalog.connections.insert(ungrouped.id, ungrouped.clone());
    catalog.connections.insert(grouped.id, grouped.clone());

    let mut sidebar = SidebarViewModel::new(catalog);
    let rows = sidebar.rows();
    assert!(matches!(rows[0], SidebarRow::Connection { id, depth: 0, .. } if id == ungrouped.id));
    assert!(matches!(rows[1], SidebarRow::Group { id, depth: 0, .. } if id == root_group.id));
    assert!(matches!(rows[2], SidebarRow::Group { id, depth: 1, .. } if id == child_group.id));
    assert!(matches!(rows[3], SidebarRow::Connection { id, depth: 2, .. } if id == grouped.id));
    assert!(rows[3].tags().contains("production"));

    sidebar.set_query("ÅNGSTRÖM");
    assert_eq!(sidebar.connection_ids(), vec![ungrouped.id]);
    sidebar.set_query("运维");
    assert_eq!(sidebar.connection_ids(), vec![grouped.id]);
    assert!(
        sidebar
            .rows()
            .iter()
            .any(|row| matches!(row, SidebarRow::Group { id, .. } if *id == child_group.id)),
        "search keeps the matching connection's group context"
    );
}

#[test]
fn sidebar_actions_emit_exact_locked_catalog_commands() {
    let connection = profile("Source", "source.example.test");
    let destination = ConnectionGroup::new("Destination");
    let tags = BTreeSet::from(["alpha".into(), "beta".into()]);

    let cases = [
        SidebarAction::Search("prod".into()).into_command(),
        SidebarAction::Duplicate {
            source: connection.id,
            destination: Some(destination.id),
        }
        .into_command(),
        SidebarAction::MoveConnection {
            connection: connection.id,
            destination: Some(destination.id),
            position: 3,
        }
        .into_command(),
        SidebarAction::DeleteConnection(connection.id).into_command(),
        SidebarAction::CreateGroup(destination.clone()).into_command(),
        SidebarAction::RenameGroup {
            group: destination.id,
            name: "Renamed".into(),
        }
        .into_command(),
        SidebarAction::MoveGroup {
            group: destination.id,
            parent: None,
            position: 2,
        }
        .into_command(),
        SidebarAction::DeleteGroup(destination.id).into_command(),
        SidebarAction::SetTags {
            connection: connection.id,
            tags: tags.clone(),
        }
        .into_command(),
    ];

    assert!(matches!(&cases[0], UiCommand::SearchConnections(query) if query == "prod"));
    assert!(matches!(
        &cases[1],
        UiCommand::ApplyCatalog {
            mutation: CatalogMutation::Duplicate { source, destination: Some(group) },
            secret: SecretUpdate::Unchanged,
        } if *source == connection.id && *group == destination.id
    ));
    assert!(matches!(
        &cases[2],
        UiCommand::ApplyCatalog {
            mutation: CatalogMutation::Move { connection: id, destination: Some(group), position: 3 },
            secret: SecretUpdate::Unchanged,
        } if *id == connection.id && *group == destination.id
    ));
    assert!(matches!(
        &cases[3],
        UiCommand::ApplyCatalog {
            mutation: CatalogMutation::Delete(id),
            secret: SecretUpdate::Unchanged,
        } if *id == connection.id
    ));
    assert!(
        matches!(&cases[4], UiCommand::ApplyCatalog { mutation: CatalogMutation::CreateGroup(group), .. } if group.id == destination.id)
    );
    assert!(
        matches!(&cases[5], UiCommand::ApplyCatalog { mutation: CatalogMutation::RenameGroup { group, name }, .. } if *group == destination.id && name == "Renamed")
    );
    assert!(
        matches!(&cases[6], UiCommand::ApplyCatalog { mutation: CatalogMutation::MoveGroup { group, parent: None, position: 2 }, .. } if *group == destination.id)
    );
    assert!(
        matches!(&cases[7], UiCommand::ApplyCatalog { mutation: CatalogMutation::DeleteGroup(group), .. } if *group == destination.id)
    );
    assert!(
        matches!(&cases[8], UiCommand::ApplyCatalog { mutation: CatalogMutation::SetTags { connection: id, tags: actual }, .. } if *id == connection.id && actual == &tags)
    );
}
