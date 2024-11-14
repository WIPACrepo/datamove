// disk_archiver.rs

use log::{info, warn};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::time::{sleep, Duration};

use crate::sps::{context::Context, database::ensure_host, models::JadeHost};

pub type Error = Box<dyn core::error::Error>;
pub type Result<T> = core::result::Result<T, Error>;

pub struct DiskArchiver {
    pub context: Context,
    pub host: Option<JadeHost>,
    pub shutdown: Arc<AtomicBool>,
}

impl DiskArchiver {
    pub fn new(context: Context) -> Self {
        Self {
            context,
            host: None,
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        // ensure host
        let jade_host = ensure_host(&mut self.context)?;
        self.host = Some(jade_host);

        // until a shutdown is requested
        while !self.shutdown.load(Ordering::Relaxed) {
            // perform the work of the disk archiver
            do_work_cycle(self).await?;
            // sleep until the next work cycle
            let work_cycle_sleep_seconds = self
                .context
                .config
                .sps_disk_archiver
                .work_cycle_sleep_seconds;
            info!("Will sleep for {} seconds.", work_cycle_sleep_seconds);
            sleep(Duration::from_secs(work_cycle_sleep_seconds)).await;
        }

        // we received a shutdown command
        info!("Initiating graceful shutdown of DiskArchiver.");
        Ok(())
    }

    pub fn request_shutdown(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }
}

pub async fn do_work_cycle(_disk_archiver: &DiskArchiver) -> Result<()> {
    // start the work cycle
    info!("Starting DiskArchiver work cycle.");

    warn!("Doing some important DiskArchiver work here.");

    // finish the work cycle successfully
    info!("End of DiskArchiver work cycle.");
    Ok(())
}

//---------------------------------------------------------------------------
//---------------------------------------------------------------------------
//---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    // use super::*;

    #[test]
    fn test_always_succeed() {
        assert!(true);
    }
}
