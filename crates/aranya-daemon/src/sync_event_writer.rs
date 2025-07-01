//! Sync event writer for coordinator animations
//! 
//! Writes sync events to a file that the coordinator can monitor

use std::path::PathBuf;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tracing::{error, debug};
use crate::sync::task::{SyncEvent, SyncEventSender};

pub struct SyncEventWriter {
    event_file: PathBuf,
    rx: mpsc::UnboundedReceiver<SyncEvent>,
}

impl SyncEventWriter {
    pub fn new(work_dir: &PathBuf) -> (SyncEventSender, Self) {
        let (tx, rx) = mpsc::unbounded_channel();
        let event_file = work_dir.join("sync_events.log");
        
        (tx, Self { event_file, rx })
    }
    
    pub async fn run(mut self) {
        while let Some(event) = self.rx.recv().await {
            if let Err(e) = self.write_event(&event).await {
                error!("Failed to write sync event: {}", e);
            }
        }
    }
    
    async fn write_event(&self, event: &SyncEvent) -> std::io::Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.event_file)
            .await?;
            
        let line = format!(
            "{},{},{},{}\n",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis(),
            event.peer_addr,
            hex::encode(event.graph_id.into_id().as_bytes()),
            event.commands_count
        );
        
        file.write_all(line.as_bytes()).await?;
        file.flush().await?;
        debug!("Wrote sync event to file: {:?}", event);
        Ok(())
    }
}