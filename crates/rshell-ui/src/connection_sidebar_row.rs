use gtk::prelude::*;
use relm4::gtk;

use crate::SidebarRow;

pub(crate) fn sidebar_row(row: &SidebarRow) -> gtk::ListBoxRow {
    let container = gtk::Box::new(gtk::Orientation::Vertical, 2);
    match row {
        SidebarRow::Group { depth, name, .. } => {
            container.set_margin_start((*depth as i32) * 12);
            let label = gtk::Label::new(Some(name));
            label.set_halign(gtk::Align::Start);
            label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            label.add_css_class("folder-header");
            container.append(&label);
        }
        SidebarRow::Connection {
            depth,
            name,
            metadata,
            tags,
            ..
        } => {
            container.set_margin_start((*depth as i32) * 12);
            container.add_css_class("connection-row");
            for (text, class) in [
                (name.as_str(), "connection-name"),
                (metadata.as_str(), "connection-meta"),
            ] {
                let label = gtk::Label::new(Some(text));
                label.set_halign(gtk::Align::Start);
                label.set_ellipsize(gtk::pango::EllipsizeMode::End);
                label.add_css_class(class);
                container.append(&label);
            }
            if !tags.is_empty() {
                let text = tags.iter().cloned().collect::<Vec<_>>().join(" · ");
                let label = gtk::Label::new(Some(&text));
                label.set_halign(gtk::Align::Start);
                label.set_ellipsize(gtk::pango::EllipsizeMode::End);
                label.add_css_class("connection-meta");
                container.append(&label);
            }
        }
    }
    let list_row = gtk::ListBoxRow::builder().child(&container).build();
    list_row.add_css_class("navigation-row");
    list_row
}
