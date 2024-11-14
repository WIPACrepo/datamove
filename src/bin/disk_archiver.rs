// disk_archiver.rs

// #![feature(trivial_bounds)]
#![forbid(unsafe_code)]

use log::{error, info, warn};
use std::sync::atomic::Ordering;
use tokio::time::{sleep, Duration};

use wipac_datamove::sps::{context::load_context, process::disk_archiver::DiskArchiver};

#[tokio::main]
async fn main() {
    // set up logging
    env_logger::init();
    info!("Hello, disk-archiver!");

    // create a new DiskArchiver
    let context = load_context();
    let mut disk_archiver = DiskArchiver::new(context);

    // tell the DiskArchiver to shut down in ~30 seconds
    let shutdown = disk_archiver.shutdown.clone();
    tokio::spawn(async move {
        sleep(Duration::from_secs(30)).await;
        warn!("Shutting down the DiskArchiver.");
        shutdown.store(true, Ordering::Relaxed);
    });

    // log about the final result
    match disk_archiver.run().await {
        Ok(_) => info!("DiskArchiver ends successfully."),
        Err(e) => error!("DiskArchiver ends with an error: {e}"),
    }
}
