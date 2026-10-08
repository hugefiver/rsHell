#![cfg(not(target_os = "macos"))]
use gtk::prelude::*;
use relm4::{Component, ComponentController};
use rshell_core::{PaneId, PaneTree, TabId, TabState, UiCommand, WorkspaceState};
use rshell_ui::{SessionTabBar, SessionTabBarInit, SessionTabBarOutput};
use std::{cell::RefCell, rc::Rc};

#[path = "support/terminal_tab_group_assertions.rs"]
mod assertions;
#[allow(dead_code, unused_imports)]
#[path = "support/fluent_native.rs"]
mod fluent_native;
#[path = "support/terminal_tab_group.rs"]
mod support;
use assertions::*;
use support::*;

#[test]
fn native_shared_tab_boundary_and_identity() {
    gtk::init().expect("native tab regression requires GTK display");
    for (mode, width, height) in [
        ("compact", 800, 600),
        ("standard", 1360, 860),
        ("wide", 1920, 1080),
    ] {
        run(mode, width, height, 2);
        run(mode, width, height, 20);
    }
}

fn run(mode: &str, width: i32, height: i32, count: usize) {
    let (main, owned) = launch(width, height);
    let old = descendants(main.widget())
        .into_iter()
        .find(|w| w.has_css_class("tab-bar"))
        .unwrap();
    let parent = old.parent().unwrap().downcast::<gtk::Box>().unwrap();
    let tabs = (0..count)
        .map(|i| {
            let pane = PaneId::new();
            TabState {
                id: TabId::new_v4(),
                title: if i == 0 {
                    "Local terminal".into()
                } else {
                    format!("Tab {:02}", i + 1)
                },
                pane_tree: PaneTree::leaf(pane),
                active_pane: pane,
            }
        })
        .collect::<Vec<_>>();
    let ids = tabs.iter().map(|t| t.id).collect::<Vec<_>>();
    let outputs = Rc::new(RefCell::new(Vec::new()));
    let recorded = outputs.clone();
    let bar = SessionTabBar::builder()
        .launch(SessionTabBarInit {
            workspace: WorkspaceState {
                tabs,
                active_tab: Some(ids[0]),
            },
        })
        .connect_receiver(move |_, output| recorded.borrow_mut().push(output));
    parent.insert_child_after(bar.widget(), Some(&old));
    parent.remove(&old);
    wait(|| bar.widget().is_mapped() && active_group(bar.widget()).height() > 0);
    fluent_native::wait_for_frame(main.widget(), "tab strip painted", |_| true);
    gtk::prelude::GtkWindowExt::set_focus(main.widget(), None::<&gtk::Widget>);
    fluent_native::wait_for_frame(main.widget(), "unfocused selection", |_| true);
    let group = active_group(bar.widget());
    if count == 2 {
        let stage = if group.has_css_class("active-tab") {
            "green"
        } else {
            "red"
        };
        capture(bar.widget(), &format!("{mode}-{stage}"));
    }
    assert_boundary(&group, mode);
    assert!(group.has_css_class("active-tab"));
    let second = button(bar.widget(), "Activate Tab 02 tab");
    assert_eq!(second.accessible_role(), gtk::AccessibleRole::Button);
    assert!(second.grab_focus());
    assert_eq!(
        gtk::prelude::RootExt::focus(main.widget()).as_ref(),
        Some(second.upcast_ref())
    );
    fluent_native::wait_for_frame(main.widget(), "independent title focus", |_| true);
    let (rgba, w, _) = pixels(&second);
    assert!(
        rgba.chunks_exact(w * 4).take(2).all(|row| row
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| accent(p)))
    );
    assert!(second.activate(), "native Root button activation");
    wait(|| {
        outputs
            .borrow()
            .iter()
            .any(|o| matches!(o, SessionTabBarOutput::ActivateTab(id) if *id == ids[1]))
    });
    wait(|| active_group(bar.widget()) != group);
    fluent_native::wait_for_frame(main.widget(), "second tab active", |_| true);
    assert!(
        !group.is_mapped(),
        "old active wrapper detached after render"
    );
    gtk::prelude::GtkWindowExt::set_focus(main.widget(), None::<&gtk::Widget>);
    fluent_native::wait_for_frame(main.widget(), "second tab selection painted", |_| true);
    let second_group = active_group(bar.widget());
    assert_boundary(&second_group, mode);
    for inactive in descendants(bar.widget())
        .into_iter()
        .filter(|w| w.has_css_class("terminal-tab") && *w != second_group)
    {
        assert!(!inactive.has_css_class("active-tab"));
        let (rgba, w, h) = pixels(&inactive);
        assert!(
            !rgba[(h - 2) * w * 4..]
                .as_chunks::<4>()
                .0
                .iter()
                .any(|p| accent(p))
        );
    }
    if count == 2 {
        capture(bar.widget(), &format!("{mode}-second"));
        assert_close_states(bar.widget(), main.widget(), mode);
    } else {
        assert_overflow(bar.widget(), main.widget(), &outputs, &ids, mode);
    }
    let target = if count == 2 { 1 } else { 19 };
    let close = button(bar.widget(), &format!("Close Tab {:02} tab", target + 1));
    close.emit_clicked();
    wait(|| {
        outputs.borrow().iter().any(|o| matches!(o, SessionTabBarOutput::Command(c) if matches!(c.as_ref(), UiCommand::CloseTab(id) if *id == ids[target])))
    });
    println!(
        "TAB_ACTIONS mode={mode} count={count} activation=true close_identity=true physical_input=false"
    );
    drop(owned);
}
