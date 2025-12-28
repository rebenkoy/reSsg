use std::path::PathBuf;
use std::sync::mpsc::TryRecvError;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use anyhow::{anyhow, Context, Error};
use futures::FutureExt;
use crate::config::reSsgConfig;
use notify_debouncer_full::{new_debouncer, notify::RecursiveMode, DebounceEventHandler, DebounceEventResult, DebouncedEvent, Debouncer, RecommendedCache};
use async_std;
use itertools::Itertools;
use notify::{Event, EventKind, RecommendedWatcher};
use crate::build::build;

pub struct Tx(pub flume::Sender<DebounceEventResult>);

impl DebounceEventHandler for Tx {
    fn handle_event(&mut self, event: DebounceEventResult) {
        self.0.send(event).unwrap();
    }
}

pub type EmittedEvent = Vec<(EventKind, Vec<PathBuf>)>;


async fn changes_handler(events: EmittedEvent, socket_sender: &mut flume::Sender<EmittedEvent>, config: &reSsgConfig, fs: &Arc<RwLock<rsfs::mem::FS>>) -> anyhow::Result<()> {
    let mut new_fs = rsfs::mem::FS::new();
    match build(&config.build, &mut new_fs).map_err(|e| anyhow!(e.to_string())) {
        Ok(_) => {}
        Err(e) => {
            log::error!("Error while building new files: {}", e);
        }
    }
    let mut guard = fs.write().map_err(|e| anyhow!(e.to_string()))?;
    *guard = new_fs;
    socket_sender.send_async(events).await?;
    Ok(())
}

pub fn build_watcher_tread(config: &reSsgConfig, fs: Arc<RwLock<rsfs::mem::FS>>) -> anyhow::Result<(flume::Receiver<EmittedEvent>, impl Future<Output = anyhow::Result<()>>)> {
    let (socket_sender, socket_reciever) = flume::unbounded();

    let (tx, rx) = flume::unbounded();
    let mut debouncer = new_debouncer(Duration::from_secs(1), None, Tx(tx))
        .map_err(anyhow::Error::from)?;
    debouncer
        .watch(".", RecursiveMode::Recursive)
        .with_context(|| "Can't watch for changes in project root folder. Does it exist, and do you have correct permissions?".to_string())?;

    async fn fun(rx: flume::Receiver<DebounceEventResult>, mut tx: flume::Sender<EmittedEvent>, config: &reSsgConfig, fs: Arc<RwLock<rsfs::mem::FS>>, guard: Debouncer<RecommendedWatcher, RecommendedCache>) -> anyhow::Result<()> {
        loop {
            match rx.recv_async().await {
                Ok(Ok(events)) => {
                    let events = events
                        .into_iter()
                        .map(|e| {
                            let DebouncedEvent { event, time } = e;
                            let Event { kind, paths, attrs } = event;
                            (kind, paths)
                        })
                        .filter(|(kind, paths)| {
                            match kind {
                                EventKind::Access(_) => false,
                                EventKind::Other => false,
                                _ => true,

                            }
                        })
                        .collect_vec();
                    if !events.is_empty() {
                        changes_handler(
                            events,
                            &mut tx,
                            &config,
                            &fs,
                        ).await?;
                    }
                }
                Ok(Err(err)) => {
                    for err in err {
                        log::error!("watch error: {:?}", err);
                    }
                }
                Err(flume::RecvError::Disconnected) => break,
            }
        };
        Ok(())
    }
    Ok((socket_reciever, fun(rx, socket_sender, &config, fs, debouncer)))
}