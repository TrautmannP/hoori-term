//! Coalesced wakeups, separate from the bounded PTY byte queue.
use iced::futures::channel::mpsc;
use iced::futures::{Stream, StreamExt, stream};
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
pub struct Notifications {
    id: u64,
    sender: Arc<Mutex<mpsc::Sender<()>>>,
    receiver: Arc<Mutex<Option<mpsc::Receiver<()>>>>,
}

impl Default for Notifications {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel(1);
        Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            sender: Arc::new(Mutex::new(sender)),
            receiver: Arc::new(Mutex::new(Some(receiver))),
        }
    }
}

impl Hash for Notifications {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl Notifications {
    pub fn wake(&self) {
        // A full wakeup queue already guarantees a pending UI refresh.
        let _ = self.sender.lock().unwrap().try_send(());
    }

    pub fn stream(&self) -> impl Stream<Item = ()> + Send + 'static + use<> {
        let receiver = self
            .receiver
            .lock()
            .unwrap()
            .take()
            .expect("one subscription per notification source");
        stream::unfold(receiver, |mut receiver| async move {
            receiver.next().await?;
            // At most one output refresh per frame, with no idle UI polling.
            tokio::time::sleep(Duration::from_millis(16)).await;
            Some(((), receiver))
        })
    }
}
