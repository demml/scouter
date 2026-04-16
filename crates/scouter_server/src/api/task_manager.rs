use futures::Future;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::Duration;

pub struct TaskManager {
    tasks: Vec<JoinHandle<()>>,
    shutdown_tx: watch::Sender<()>,
}

impl TaskManager {
    pub fn new() -> Self {
        let (shutdown_tx, _) = watch::channel(());
        Self {
            tasks: Vec::new(),
            shutdown_tx,
        }
    }

    pub fn get_shutdown_receiver(&self) -> watch::Receiver<()> {
        self.shutdown_tx.subscribe()
    }

    pub fn spawn<F>(&mut self, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.tasks.push(tokio::spawn(future));
    }

    pub async fn shutdown(&self) {
        let _ = self.shutdown_tx.send(());
        // Allow workers time to drain in-flight and queued messages before aborting.
        // Workers drain their local channel queue on shutdown, so 5s covers all but
        // pathologically slow DB inserts.
        tokio::time::sleep(Duration::from_secs(5)).await;
        for task in &self.tasks {
            task.abort();
        }
    }
}

impl Default for TaskManager {
    fn default() -> Self {
        Self::new()
    }
}
