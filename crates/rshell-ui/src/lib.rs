//! Native GTK4/Relm4 presentation components for rsHell.

mod command_port;
mod connection_editor;
mod connection_editor_bindings;
mod connection_editor_diagnostics;
#[cfg(test)]
mod connection_editor_diagnostics_tests;
mod connection_editor_message;
mod connection_editor_override_bindings;
mod connection_editor_override_render;
mod connection_editor_override_state;
mod connection_editor_override_updates;
mod connection_editor_override_widgets;
mod connection_editor_render;
mod connection_editor_snapshot;
mod connection_editor_state;
mod connection_editor_widgets;
mod connection_sidebar;
mod connection_sidebar_row;
mod connection_sidebar_selection;
mod connection_sidebar_widgets;
mod file_selector;
mod icon_backend;
mod icon_cache;
mod icon_registry;
mod icon_vector;
mod icon_vector_data;
mod import_dialog;
mod import_dialog_message;
mod import_dialog_render;
mod import_dialog_widgets;
mod import_view_model;
mod interaction_dialog;
mod interaction_dialog_message;
mod interaction_dialog_queue;
mod interaction_dialog_render;
mod interaction_dialog_widgets;
mod interaction_view_model;
mod key_binding_text;
mod main_window;
mod main_window_commands;
mod main_window_dialog_events;
mod main_window_dialogs;
mod main_window_events;
mod main_window_init;
mod main_window_layout;
mod main_window_smoke;
mod main_window_smoke_binding;
#[cfg(test)]
mod main_window_smoke_binding_tests;
mod main_window_smoke_capture;
mod main_window_smoke_close;
mod main_window_smoke_evidence;
mod main_window_smoke_input;
mod main_window_smoke_observation;
mod main_window_smoke_resize;
mod main_window_smoke_routes;
mod main_window_smoke_terminal_effects;
#[cfg(test)]
mod main_window_smoke_tests;
mod main_window_smoke_visual;
mod main_window_smoke_workflow_evidence;
mod main_window_snapshots;
mod main_window_streams;
mod pane_host;
mod pane_host_init;
mod pane_host_model;
mod pane_host_render;
mod pane_host_terminals;
mod pane_projection;
mod pane_view_model;
mod session_diagnostics;
#[cfg(test)]
mod session_diagnostics_tests;
mod session_tab_bar;
mod settings_view_model;
mod settings_window;
mod settings_window_message;
mod settings_window_render;
mod settings_window_widgets;
mod smoke_driver;
mod smoke_driver_action_kind;
mod smoke_driver_actions;
#[cfg(test)]
mod smoke_driver_auth_tests;
mod smoke_driver_completion;
#[cfg(test)]
mod smoke_driver_completion_tests;
mod smoke_driver_evidence;
mod smoke_driver_failure;
mod smoke_driver_observation;
mod smoke_driver_progress;
#[cfg(test)]
mod smoke_driver_progress_tests;
mod smoke_driver_report;
mod smoke_driver_routing;
mod smoke_driver_sequences;
mod smoke_driver_state;
#[cfg(test)]
mod smoke_driver_state_tests;
#[cfg(test)]
mod smoke_driver_terminal_tests;
#[cfg(test)]
mod smoke_driver_visual_tests;
mod startup_probe;
mod terminal_frame;
mod terminal_geometry;
mod terminal_input;
mod terminal_paint;
mod terminal_palette;
mod terminal_render_cache;
mod terminal_renderer;
mod terminal_search;
mod terminal_view;
mod terminal_view_keys;
mod terminal_view_message;
mod terminal_view_model;
mod terminal_view_pointer;
mod terminal_view_widgets;
mod theme;
mod view_model;
mod visual_contract;
mod visual_png;

pub use connection_editor::{
    ConnectionEditor, ConnectionEditorDraftState, ConnectionEditorInit, ConnectionEditorMsg,
    ConnectionEditorOutput, ConnectionEditorState, EditorTextField,
};
pub use connection_sidebar::{
    ConnectionSidebar, ConnectionSidebarInit, ConnectionSidebarMsg, ConnectionSidebarOutput,
};
pub use file_selector::GtkFileSelectionService;
pub use icon_cache::embedded_icons_ready;
pub use icon_registry::{
    IconBackend, IconMetadata, IconRenderError, ProductIcon, SvgDecodeOutcome,
};
pub use import_dialog::{
    ImportDialog, ImportDialogInit, ImportDialogMsg, ImportDialogOutput, ImportDialogState,
};
pub use import_view_model::ImportViewModel;
pub use interaction_dialog::{
    InteractionDialog, InteractionDialogInit, InteractionDialogMsg, InteractionDialogOutput,
    InteractionDialogState,
};
pub use interaction_view_model::{InteractionAction, InteractionViewModel};
pub use main_window::{MainWindow, MainWindowMsg};
pub use main_window_init::MainWindowInit;
pub use pane_host::{PaneHost, PaneHostMsg, PaneHostOutput};
pub use pane_host_init::PaneHostInit;
pub use pane_host_model::PaneHostModel;
pub use pane_projection::PaneProjection;
pub use pane_view_model::{PaneAction, PanePageKind, SessionPaneViewModel};
pub use session_tab_bar::{
    SessionTabBar, SessionTabBarAction, SessionTabBarInit, SessionTabBarMsg, SessionTabBarOutput,
};
pub use settings_view_model::SettingsViewModel;
pub use settings_window::{
    SettingsBoolField, SettingsTextField, SettingsWindow, SettingsWindowInit, SettingsWindowMsg,
    SettingsWindowOutput,
};
pub use smoke_driver::{
    DEFAULT_SMOKE_SCENARIO_TIMEOUT, DEFAULT_SMOKE_STEP_TIMEOUT, SMOKE_SCENARIO_VERSION,
    SmokeAction, SmokeActionKind, SmokeConnectionField, SmokeDriverInit, SmokeImportExpectation,
    SmokeScenario, SmokeScenarioError, SmokeStep,
};
pub use smoke_driver_report::{
    SmokeBindingEvidence, SmokeCellRangeEvidence, SmokeClipboardEvidence, SmokeColorEvidence,
    SmokeCounters, SmokeFailure, SmokeFieldStatus, SmokeFrameEvidence, SmokeImportEvidence,
    SmokeImportPreviewEvidence, SmokePasteEvidence, SmokeReconnectEvidence, SmokeReport,
    SmokeReportHandle, SmokeResizeEvidence, SmokeScenarioState, SmokeSearchEvidence,
    SmokeSelectionEvidence, SmokeStepReport, SmokeStepState, SmokeTerminalEvidence,
};
pub use startup_probe::{StartupProbe, StartupReport};
pub use terminal_geometry::{PointerEvent, ViewRect};
pub use terminal_input::{FontMetrics, TerminalViewError, map_gdk_key};
pub use terminal_render_cache::TerminalRenderCache;
pub use terminal_renderer::{TerminalDecorations, TerminalDrawStats, TerminalRenderer};
pub use terminal_view::{TerminalView, TerminalViewInit, TerminalViewMsg, TerminalViewOutput};
pub use terminal_view_model::{FrameUpdate, TerminalClipboardAction, TerminalViewModel};
pub use theme::{apply_global_css, embedded_theme_css};
pub use view_model::{
    AuthenticationCapabilities, ConnectionEditorDraft, ConnectionEditorViewModel,
    EditorValidationError, SecretEditKind, SidebarAction, SidebarRow, SidebarViewModel,
    TerminalOverrideKey,
};
pub use visual_contract::{
    SmokePngEvidence, SmokeVisualEvidence, SmokeVisualFacts, collect_visual_facts,
    selection_treatment_surface,
};
pub use visual_png::{
    NativeByteOrder, analyze_rgba, analyze_rgba_with_accent, argb32_native_to_rgba,
};
