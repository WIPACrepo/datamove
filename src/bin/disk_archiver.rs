// disk_archiver.rs

// #![feature(trivial_bounds)]
#![forbid(unsafe_code)]

use axum::{routing::get, routing::post, Router};
use log::{error, info};
use std::sync::Arc;
use tokio::{net::TcpListener, sync::Notify, try_join};

use wipac_datamove::sps::{context::load_context, process::disk_archiver::DiskArchiver};
use wipac_datamove::status::net::{get_status_disk_archiver, post_shutdown_disk_archiver};

#[tokio::main]
async fn main() {
    // set up logging
    env_logger::init();
    info!("Hello, disk-archiver!");

    // create a new DiskArchiver
    let context = load_context();
    let status_port = context.config.sps_disk_archiver.status_port;
    let disk_archiver = Arc::new(DiskArchiver::new(context).await);
    let shutdown_notify = Arc::new(Notify::new());

    // start an Axum server to provide JSON status responses
    let da = Arc::clone(&disk_archiver);
    let sn_await = Arc::clone(&shutdown_notify);
    let sn_notify = Arc::clone(&shutdown_notify);
    let handle_axum_server = tokio::spawn(async move {
        // establish our listening port
        let listener = TcpListener::bind(format!("0.0.0.0:{}", status_port))
            .await
            .unwrap_or_else(|_| panic!("Unable to listen on port {}", status_port));
        // build our status serving route(s)
        let app: Router = Router::new()
            .route(
                "/shutdown",
                post(move |state| post_shutdown_disk_archiver(state, sn_notify)),
            )
            .route("/status", get(get_status_disk_archiver))
            .with_state(da);
        // start the status service
        info!(
            "DiskArchiver status service: http://{}/status",
            listener.local_addr().unwrap()
        );
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                sn_await.notified().await;
            })
            .await
            .unwrap();
    });

    // start the DiskArchiver daemon process
    let da = Arc::clone(&disk_archiver);
    let handle_jade_process = tokio::spawn(async move {
        da.run().await.unwrap();
    });

    // wait for both tasks to complete, if either one has an error
    if let Err(e) = try_join!(handle_axum_server, handle_jade_process) {
        // log and print the error
        error!("An error occurred: {}", e);
        eprintln!("An error occurred: {:?}", e);
    }

    // log about the fact that the DiskArchiver has finally shut down
    info!("DiskArchiver has shut down.");
}
