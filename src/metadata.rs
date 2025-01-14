// metadata.rs

use serde::{Deserialize, Serialize};

use crate::sps::jade_db::service::disk::JadeDisk;

/// metadata for an archival disk
#[derive(Clone, Deserialize, Serialize)]
pub struct ArchivalDiskMetadata {
    #[serde(rename = "capacity")]
    pub capacity: i64,

    #[serde(rename = "copyId")]
    pub copy_id: i32,

    #[serde(rename = "dateCreated")]
    pub date_created: i64,

    #[serde(rename = "dateUpdated")]
    pub date_updated: i64,

    #[serde(rename = "diskArchiveUuid")]
    pub disk_archive_uuid: String,

    #[serde(rename = "id")]
    pub id: i64,

    #[serde(rename = "label")]
    pub label: String,

    #[serde(rename = "uuid")]
    pub uuid: String,
}

impl From<&JadeDisk> for ArchivalDiskMetadata {
    fn from(value: &JadeDisk) -> Self {
        ArchivalDiskMetadata {
            capacity: value.capacity,
            copy_id: value.copy_id,
            date_created: value.date_created.and_utc().timestamp_millis(),
            date_updated: value.date_updated.and_utc().timestamp_millis(),
            disk_archive_uuid: value.disk_archive_uuid.clone(),
            id: value.jade_disk_id,
            label: value.label.clone(),
            uuid: value.uuid.clone(),
        }
    }
}

/// metadata for a file stored on an archival disk
#[derive(Clone, Deserialize, Serialize)]
pub struct ArchivalDiskFile {
    #[serde(rename = "archiveChecksum")]
    pub archive_checksum: String,

    #[serde(rename = "archiveFile")]
    pub archive_file: String,

    #[serde(rename = "archiveSize")]
    pub archive_size: i64,

    #[serde(rename = "binaryFile")]
    pub binary_file: String,

    #[serde(rename = "binarySize")]
    pub binary_size: i64,

    #[serde(rename = "dataStreamId")]
    pub data_stream_id: i64,

    #[serde(rename = "dataStreamUuid")]
    pub data_stream_uuid: String,

    #[serde(rename = "dataWarehousePath")]
    pub data_warehouse_path: String,

    #[serde(rename = "dateCreated")]
    pub date_created: i64,

    #[serde(rename = "dateFetched")]
    pub date_fetched: i64,

    #[serde(rename = "dateProcessed")]
    pub date_processed: i64,

    #[serde(rename = "dateUpdated")]
    pub date_updated: i64,

    #[serde(rename = "dateVerified")]
    pub date_verified: i64,

    // TODO: Do we want a JSON form of the XML metadata that we never use?
    // #[serde(rename = "DIF_Plus")]
    // pub dif_plus: DIFPlus,
    #[serde(rename = "diskCount")]
    pub disk_count: i32,

    #[serde(rename = "fetchChecksum")]
    pub fetch_checksum: String,

    #[serde(rename = "fetchedByHost")]
    pub fetched_by_host: String,

    #[serde(rename = "fingerprint")]
    pub fingerprint: String,

    #[serde(rename = "metadataFile")]
    pub metadata_file: String,

    #[serde(rename = "originChecksum")]
    pub origin_checksum: String,

    #[serde(rename = "originModificationDate")]
    pub origin_modification_date: i64,

    #[serde(rename = "semaphoreFile")]
    pub semaphore_file: String,

    #[serde(rename = "uuid")]
    pub uuid: String,
    // TODO: Do we want an extra copy of the XML metadata that we never use?
    //       Note, it's also in the JADE database and the file pair archive.
    // #[serde(rename = "xmlMetadata")]
    // pub xml_metadata: String,
}
