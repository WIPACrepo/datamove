// net.rs

use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse};
use axum_extra::response::ErasedJson;
use serde_json::json;
use tokio::sync::Notify;
use tracing::warn;

use crate::sps::process::disk_archiver::DiskArchiver;

pub async fn get_status_disk_archiver(State(state): State<Arc<DiskArchiver>>) -> impl IntoResponse {
    let disk_archiver_status = state.get_status().await;
    ErasedJson::pretty(disk_archiver_status)
}

pub async fn post_shutdown_disk_archiver(
    State(state): State<Arc<DiskArchiver>>,
    shutdown_notify: Arc<Notify>,
) -> impl IntoResponse {
    // signal the DiskArchiver to shutdown
    state.request_shutdown();
    // signal the Axum status server to shutdown
    shutdown_notify.notify_one();
    // log about the fact that we're shutting down
    warn!("Shutdown requested for DiskArchiver.");
    // tell the caller that shutdown has been initiated
    (
        StatusCode::ACCEPTED,
        ErasedJson::pretty(json!({
            "status": "shutdown initiated"
        })),
    )
}
