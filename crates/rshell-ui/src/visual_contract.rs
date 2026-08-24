use gtk::prelude::*;
use relm4::gtk;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SmokeVisualFacts {
    pub requested_width: i32,
    pub requested_height: i32,
    pub realized_width: i32,
    pub realized_height: i32,
    pub command_bar: bool,
    pub dense_sidebar: bool,
    pub tab_strip: bool,
    pub pane_command_row: bool,
    pub terminal_canvas: bool,
    pub content_dialog: bool,
    pub embedded_icon_count: usize,
    pub focus_or_selection_treatment: bool,
}

impl SmokeVisualFacts {
    pub const fn contract_passes(self) -> bool {
        self.requested_width == 1_360
            && self.requested_height == 860
            && self.realized_width > 0
            && self.realized_height > 0
            && self.command_bar
            && self.dense_sidebar
            && self.tab_strip
            && self.pane_command_row
            && self.terminal_canvas
            && self.content_dialog
            && self.embedded_icon_count >= 6
            && self.focus_or_selection_treatment
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SmokePngEvidence {
    pub width: i32,
    pub height: i32,
    pub non_empty: bool,
    pub luminance_buckets: usize,
    pub dark_regions_required: usize,
    pub dark_regions_passed: usize,
    pub focus_or_selection_thickness_px: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SmokeVisualEvidence {
    pub facts: SmokeVisualFacts,
    pub png: Option<SmokePngEvidence>,
}

pub fn collect_visual_facts(root: &gtk::Widget, requested: (i32, i32)) -> SmokeVisualFacts {
    let widgets = descendants_including(root);
    let visible = |widget: &&gtk::Widget| widget.is_mapped();
    let has_visible_class = |class: &str| {
        widgets
            .iter()
            .filter(visible)
            .any(|widget| widget.has_css_class(class))
    };
    let sidebar = widgets
        .iter()
        .filter(visible)
        .find(|widget| widget.has_css_class("sidebar"));
    let dense_sidebar = sidebar.is_some_and(|sidebar| {
        widgets.iter().any(|widget| {
            widget.downcast_ref::<gtk::Paned>().is_some_and(|paned| {
                paned.start_child().as_ref() == Some(sidebar)
                    && (200..=280).contains(&paned.position())
            })
        })
    });
    let tab_strip = has_visible_class("tab-bar") && has_visible_class("terminal-tab");
    let embedded_icon_count = widgets
        .iter()
        .filter(visible)
        .filter(|widget| widget.has_css_class("product-icon"))
        .count();
    let focus_or_selection_treatment = widgets.iter().filter(visible).any(|widget| {
        ["active-tab", "active-pane", "navigation-selected"]
            .into_iter()
            .any(|class| widget.has_css_class(class))
    });
    SmokeVisualFacts {
        requested_width: requested.0,
        requested_height: requested.1,
        realized_width: root.width(),
        realized_height: root.height(),
        command_bar: has_visible_class("command-bar"),
        dense_sidebar,
        tab_strip,
        pane_command_row: has_visible_class("pane-command-row"),
        terminal_canvas: has_visible_class("terminal-canvas"),
        content_dialog: has_visible_class("content-dialog"),
        embedded_icon_count,
        focus_or_selection_treatment,
    }
}

fn descendants_including(root: &gtk::Widget) -> Vec<gtk::Widget> {
    fn collect(widget: &gtk::Widget, output: &mut Vec<gtk::Widget>) {
        output.push(widget.clone());
        let mut child = widget.first_child();
        while let Some(current) = child {
            collect(&current, output);
            child = current.next_sibling();
        }
    }
    let mut output = Vec::new();
    collect(root, &mut output);
    output
}
