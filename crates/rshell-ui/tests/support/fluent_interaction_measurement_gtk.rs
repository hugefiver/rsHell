use super::{CHILD_CAP, Captured, Facts, Provider};
use crate::fluent_native::descendants;
use relm4::gtk::{self, prelude::*};

pub(super) struct GtkProvider {
    root: gtk::Widget,
    viewport: Option<gtk::Viewport>,
    body: Option<gtk::Widget>,
    children: Vec<gtk::Widget>,
    children_capped: bool,
}

impl GtkProvider {
    pub(super) fn new(root: &gtk::Widget) -> Self {
        let viewport = descendants(root)
            .into_iter()
            .find(|w| w.has_css_class("interaction-dialog"))
            .and_then(|modal| {
                descendants(&modal)
                    .into_iter()
                    .find(|w| w.has_css_class("modal-focus-first"))
            })
            .and_then(|first| first.ancestor(gtk::Viewport::static_type()))
            .and_then(|w| w.downcast::<gtk::Viewport>().ok());
        let body = viewport
            .as_ref()
            .and_then(|v| v.child())
            .filter(|w| w.has_css_class("dialog-body"));
        let mut children = Vec::new();
        let mut next = body.as_ref().and_then(|w| w.first_child());
        while children.len() < CHILD_CAP {
            let Some(child) = next else { break };
            next = child.next_sibling();
            children.push(child);
        }
        Self {
            root: root.clone(),
            viewport,
            body,
            children,
            children_capped: next.is_some(),
        }
    }
}

impl Provider for GtkProvider {
    fn capture(&mut self) -> Captured {
        let mut captured = Captured {
            version: [
                gtk::major_version(),
                gtk::minor_version(),
                gtk::micro_version(),
            ],
            children_capped: self.children_capped,
            ..Default::default()
        };
        if let Some(viewport) = &self.viewport {
            captured.viewport = Some(facts(viewport.upcast_ref(), &self.root));
            captured.policies_hv = Some([
                policy(viewport.hscroll_policy()),
                policy(viewport.vscroll_policy()),
            ]);
            captured.adjustment_luvp = viewport
                .vadjustment()
                .map(|a| [a.lower(), a.upper(), a.value(), a.page_size()]);
            if let Some(body) = &self.body {
                captured.body = Some(facts(body, viewport.upcast_ref()));
                for (i, child) in self.children.iter().enumerate() {
                    captured.children[i] = Some(facts(child, body));
                }
            }
        }
        captured
    }

    fn vertical_measure(&mut self, node: usize, width: i32) -> Option<[i32; 4]> {
        let widget = match node {
            1 => self.body.as_ref(),
            2..=9 => self.children.get(node - 2),
            _ => None,
        }?;
        let (minimum, natural, minimum_baseline, natural_baseline) =
            widget.measure(gtk::Orientation::Vertical, width);
        Some([minimum, natural, minimum_baseline, natural_baseline])
    }
}

fn facts(widget: &gtk::Widget, relative: &gtk::Widget) -> Facts {
    let allocation = widget.allocation();
    Facts {
        allocation: [
            allocation.x(),
            allocation.y(),
            allocation.width(),
            allocation.height(),
        ],
        content: [widget.width(), widget.height()],
        relative_outer: widget
            .compute_bounds(relative)
            .map(|b| [b.x(), b.y(), b.width(), b.height()]),
        visible: widget.is_visible(),
        mapped: widget.is_mapped(),
        request_mode: match widget.request_mode() {
            gtk::SizeRequestMode::ConstantSize => 1,
            gtk::SizeRequestMode::HeightForWidth => 2,
            gtk::SizeRequestMode::WidthForHeight => 3,
            _ => 0,
        },
    }
}

fn policy(policy: gtk::ScrollablePolicy) -> u8 {
    match policy {
        gtk::ScrollablePolicy::Minimum => 1,
        gtk::ScrollablePolicy::Natural => 2,
        _ => 0,
    }
}
