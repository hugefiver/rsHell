use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use relm4::ComponentSender;
use rshell_core::{AppEventStream, LatestViewStream};

use crate::{MainWindow, MainWindowMsg};

#[derive(Debug)]
pub(crate) struct MainWindowLiveSources {
    pub(crate) events: AppEventStream,
    pub(crate) views: LatestViewStream,
}

pub(crate) fn spawn_live_forwarders(
    sources: MainWindowLiveSources,
    sender: &ComponentSender<MainWindow>,
) -> Vec<gtk::glib::JoinHandle<()>> {
    let event_input = sender.input_sender().clone();
    let event_view = sources.views.clone();
    let events = gtk::glib::MainContext::default().spawn_local(async move {
        while let Some(event) = sources.events.recv().await {
            let pending = Arc::new(AtomicBool::new(true));
            if event_input
                .send(MainWindowMsg::LiveEvent {
                    view: Box::new(event_view.latest()),
                    event: Box::new(event),
                    pending: Arc::clone(&pending),
                })
                .is_err()
            {
                break;
            }
            wait_until_processed(&pending).await;
        }
    });

    let view_input = sender.input_sender().clone();
    let mut views = sources.views;
    let views = gtk::glib::MainContext::default().spawn_local(async move {
        while let Some(view) = views.changed().await {
            let pending = Arc::new(AtomicBool::new(true));
            if view_input
                .send(MainWindowMsg::LiveView {
                    view: Box::new(view),
                    pending: Arc::clone(&pending),
                })
                .is_err()
            {
                break;
            }
            wait_until_processed(&pending).await;
        }
    });
    vec![events, views]
}

async fn wait_until_processed(pending: &AtomicBool) {
    while pending.load(Ordering::Acquire) {
        gtk::glib::timeout_future(Duration::from_millis(1)).await;
    }
}
