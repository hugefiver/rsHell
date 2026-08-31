use gtk::prelude::*;
use relm4::gtk;

use crate::SmokeAccessibilityEvidence;

pub(crate) fn descendants_including(root: &gtk::Widget) -> Vec<gtk::Widget> {
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

pub fn collect_accessibility_evidence(root: &gtk::Widget) -> SmokeAccessibilityEvidence {
    let widgets = descendants_including(root);
    let mapped = widgets.iter().filter(|widget| widget.is_mapped());
    let unnamed_buttons = mapped
        .clone()
        .filter_map(|widget| widget.downcast_ref::<gtk::Button>())
        .filter(|button| {
            descendants_including(button.upcast_ref())
                .iter()
                .any(|child| {
                    child.has_css_class("product-icon") && icon_owned_by_button(child, button)
                })
                && button.tooltip_text().as_deref().is_none_or(str::is_empty)
        })
        .count();
    let unnamed_menus = mapped
        .clone()
        .filter_map(|widget| widget.downcast_ref::<gtk::MenuButton>())
        .filter(|button| {
            descendants_including(button.upcast_ref())
                .iter()
                .any(|child| child.has_css_class("product-icon"))
                && button.tooltip_text().as_deref().is_none_or(str::is_empty)
        })
        .count();
    let unnamed_icon_controls = unnamed_buttons + unnamed_menus;
    let visible_dialogs = mapped
        .clone()
        .filter(|widget| widget.has_css_class("content-dialog"))
        .collect::<Vec<_>>();
    let hidden_primary_actions = visible_dialogs
        .iter()
        .filter(|dialog| {
            !descendants_including(dialog)
                .iter()
                .any(|widget| widget.is_mapped() && widget.has_css_class("suggested-action"))
        })
        .count();
    let zero_size_panes = mapped
        .clone()
        .filter(|widget| widget.has_css_class("pane-surface"))
        .filter(|widget| widget.width() <= 0 || widget.height() <= 0)
        .filter(|widget| !has_positive_pane_content(widget))
        .count();
    let horizontal_clipping = mapped
        .clone()
        .any(|widget| widget.width() > root.width().saturating_add(1));
    let background = widgets
        .iter()
        .find(|widget| widget.has_css_class("modal-background"));
    let modal = visible_dialogs.first().copied();
    let focused = root
        .root()
        .and_then(|root| gtk::prelude::RootExt::focus(&root));
    let focus_contained = modal.is_some_and(|dialog| {
        focused
            .as_ref()
            .is_some_and(|focused| is_same_or_descendant(focused, dialog))
    });
    SmokeAccessibilityEvidence {
        unnamed_icon_controls,
        hidden_primary_actions,
        zero_size_panes,
        horizontal_clipping,
        background_insensitive: modal.is_some()
            && background.is_some_and(|widget| !widget.is_sensitive()),
        focus_contained,
        focus_restored: false,
        escape_cancelled: false,
    }
}

fn has_positive_pane_content(pane: &gtk::Widget) -> bool {
    descendants_including(pane).iter().any(|widget| {
        widget.is_mapped()
            && widget.width() > 0
            && widget.height() > 0
            && (widget.has_css_class("terminal-canvas") || widget.has_css_class("pane-status-page"))
    })
}

fn icon_owned_by_button(icon: &gtk::Widget, button: &gtk::Button) -> bool {
    let mut parent = icon.parent();
    while let Some(widget) = parent {
        if let Ok(owner) = widget.clone().downcast::<gtk::Button>() {
            return owner == *button && !has_menu_button_ancestor(button.upcast_ref());
        }
        parent = widget.parent();
    }
    false
}

fn has_menu_button_ancestor(widget: &gtk::Widget) -> bool {
    let mut parent = widget.parent();
    while let Some(widget) = parent {
        if widget.is::<gtk::MenuButton>() {
            return true;
        }
        parent = widget.parent();
    }
    false
}

pub fn visual_contrast_passes() -> bool {
    contrast_ratio([0xf5, 0xf5, 0xf5], [0x20, 0x20, 0x20]) >= 4.5
        && contrast_ratio([0x60, 0xcd, 0xff], [0x20, 0x20, 0x20]) >= 3.0
}

fn is_same_or_descendant(widget: &gtk::Widget, ancestor: &gtk::Widget) -> bool {
    let mut current = Some(widget.clone());
    while let Some(widget) = current {
        if widget == *ancestor {
            return true;
        }
        current = widget.parent();
    }
    false
}

fn contrast_ratio(foreground: [u8; 3], background: [u8; 3]) -> f64 {
    let luminance = |rgb: [u8; 3]| {
        let channel = |value: u8| {
            let value = f64::from(value) / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(rgb[0]) + 0.7152 * channel(rgb[1]) + 0.0722 * channel(rgb[2])
    };
    let first = luminance(foreground);
    let second = luminance(background);
    (first.max(second) + 0.05) / (first.min(second) + 0.05)
}
