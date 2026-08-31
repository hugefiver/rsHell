use gtk::prelude::*;
use relm4::gtk;

use crate::{MainWindow, SmokeVisualState, selection_treatment_surface};

impl MainWindow {
    pub(crate) fn prepare_smoke_paintable(
        &mut self,
        state: SmokeVisualState,
    ) -> Result<(), &'static str> {
        if self.smoke_state.visual_paintable.is_some() {
            self.shell.overlay.queue_resize();
            self.shell.overlay.queue_draw();
            return Ok(());
        }
        let widget: gtk::Widget = self.shell.overlay.clone().upcast();
        let paintable = gtk::WidgetPaintable::new(Some(&widget));
        let accent_paintable = selection_treatment_surface(&widget)
            .map(|accent| gtk::WidgetPaintable::new(Some(&accent)));
        if let Some(accent) = accent_paintable
            .as_ref()
            .and_then(gtk::WidgetPaintable::widget)
        {
            accent.queue_draw();
        }
        self.smoke_state.visual_paintable = Some(paintable);
        self.smoke_state.visual_accent_paintable = accent_paintable;
        widget.queue_resize();
        widget.queue_allocate();
        widget.queue_draw();
        if state == SmokeVisualState::TwentyTabs {
            let active = self
                .smoke_state
                .active_tab
                .or(self.view_model.workspace.active_tab)
                .ok_or("visual_active_tab_unavailable")?;
            self.send_tab(crate::SessionTabBarMsg::Activate(active));
            self.smoke_root()?.present();
        }
        Ok(())
    }

    pub(crate) fn smoke_root(&self) -> Result<gtk::ApplicationWindow, &'static str> {
        self.shell
            .overlay
            .root()
            .and_then(|root| root.downcast::<gtk::ApplicationWindow>().ok())
            .ok_or("visual_root_unavailable")
    }

    pub(crate) fn checkpoint_png_path(&self, id: &str) -> Result<std::path::PathBuf, &'static str> {
        let base = self
            .smoke_png_path
            .as_ref()
            .ok_or("visual_path_unavailable")?;
        let parent = base.parent().ok_or("visual_path_unavailable")?;
        let stem = base
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or("visual_path_unavailable")?;
        Ok(parent.join(format!("{stem}-{id}.png")))
    }
}
