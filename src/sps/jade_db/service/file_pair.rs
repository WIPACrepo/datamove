// file_pair.rs

use chrono::NaiveDateTime;
use sqlx::types::time::PrimitiveDateTime;
use sqlx::MySqlPool;
use tracing::error;

use crate::sps::jade_db::repo::file_pair::find_by_id as repo_find_by_id;
use crate::sps::jade_db::repo::file_pair::find_by_uuid as repo_find_by_uuid;
use crate::sps::jade_db::repo::file_pair::MySqlJadeFilePair;
use crate::sps::jade_db::utils::JadeDateNaive;

use crate::error::{DatamoveError, Result};

#[derive(Clone)]
pub struct JadeFilePair {
    pub jade_file_pair_id: i64,
    pub archive_checksum: Option<String>,
    pub archive_file: Option<String>,
    pub archive_size: Option<i64>,
    pub binary_file: Option<String>,
    pub binary_size: Option<i64>,
    pub date_archived: Option<NaiveDateTime>,
    pub date_created: Option<NaiveDateTime>,
    pub date_fetched: Option<NaiveDateTime>,
    pub date_processed: Option<NaiveDateTime>,
    pub date_updated: Option<NaiveDateTime>,
    pub date_verified: Option<NaiveDateTime>,
    pub fetch_checksum: Option<String>,
    pub fingerprint: Option<String>,
    pub ingest_checksum: Option<i64>,
    pub metadata_file: Option<String>,
    pub origin_checksum: Option<String>,
    pub date_modified_origin: Option<NaiveDateTime>,
    pub semaphore_file: Option<String>,
    pub version: Option<i64>,
    pub archived_by_host_id: Option<i64>,
    pub jade_data_stream_id: Option<i64>,
    pub fetched_by_host_id: Option<i64>,
    pub processed_by_host_id: Option<i64>,
    pub verified_by_host_id: Option<i64>,
    pub jade_data_stream_uuid: Option<String>,
    pub jade_file_pair_uuid: Option<String>,
    pub priority_group: Option<String>,
    pub data_warehouse_path: Option<String>,
}

impl From<MySqlJadeFilePair> for JadeFilePair {
    fn from(value: MySqlJadeFilePair) -> Self {
        fn convert_date(pdt: Option<PrimitiveDateTime>) -> Option<NaiveDateTime> {
            match pdt {
                Some(pdt) => {
                    let jade_date: JadeDateNaive = pdt.into();
                    let ndt: NaiveDateTime = jade_date.into();
                    Some(ndt)
                }
                None => None,
            }
        }

        JadeFilePair {
            jade_file_pair_id: value.jade_file_pair_id,
            archive_checksum: value.archive_checksum,
            archive_file: value.archive_file,
            archive_size: value.archive_size,
            binary_file: value.binary_file,
            binary_size: value.binary_size,
            date_archived: convert_date(value.date_archived),
            date_created: convert_date(value.date_created),
            date_fetched: convert_date(value.date_fetched),
            date_processed: convert_date(value.date_processed),
            date_updated: convert_date(value.date_updated),
            date_verified: convert_date(value.date_verified),
            fetch_checksum: value.fetch_checksum,
            fingerprint: value.fingerprint,
            ingest_checksum: value.ingest_checksum,
            metadata_file: value.metadata_file,
            origin_checksum: value.origin_checksum,
            date_modified_origin: convert_date(value.date_modified_origin),
            semaphore_file: value.semaphore_file,
            version: value.version,
            archived_by_host_id: value.archived_by_host_id,
            jade_data_stream_id: value.jade_data_stream_id,
            fetched_by_host_id: value.fetched_by_host_id,
            processed_by_host_id: value.processed_by_host_id,
            verified_by_host_id: value.verified_by_host_id,
            jade_data_stream_uuid: value.jade_data_stream_uuid,
            jade_file_pair_uuid: value.jade_file_pair_uuid,
            priority_group: value.priority_group,
            data_warehouse_path: value.data_warehouse_path,
        }
    }
}

pub async fn find_by_id(pool: &MySqlPool, find_id: i64) -> Result<JadeFilePair> {
    // try to locate the file pair by uuid in the database
    match repo_find_by_id(pool, find_id).await {
        // if we got a result back from the database
        Ok(maybe_file_pair) => {
            if let Some(file_pair) = maybe_file_pair {
                // convert it to a service layer JadeFilePair and return it to the caller
                let jade_file_pair: JadeFilePair = file_pair.into();
                Ok(jade_file_pair)
            } else {
                // otherwise log the missing file pair as an error and return an Err Result
                let msg = format!("Database table jade_file_pair has no entry for id '{find_id}'.");
                error!(msg);
                Err(DatamoveError::Critical(msg))
            }
        }
        // whoops, something went wrong in the database layer, better log about that
        Err(e) => {
            let msg =
                format!("Unable to read database table jade_file_pair for id '{find_id}': {e}.");
            error!(msg);
            Err(DatamoveError::Critical(msg))
        }
    }
}

pub async fn find_by_uuid(pool: &MySqlPool, find_uuid: &str) -> Result<JadeFilePair> {
    // try to locate the file pair by uuid in the database
    match repo_find_by_uuid(pool, find_uuid).await {
        // if we got a result back from the database
        Ok(maybe_file_pair) => {
            if let Some(file_pair) = maybe_file_pair {
                // convert it to a service layer JadeFilePair and return it to the caller
                let jade_file_pair: JadeFilePair = file_pair.into();
                Ok(jade_file_pair)
            } else {
                // otherwise log the missing file pair as an error and return an Err Result
                let msg =
                    format!("Database table jade_file_pair has no entry for uuid '{find_uuid}'.");
                error!(msg);
                Err(DatamoveError::Critical(msg))
            }
        }
        // whoops, something went wrong in the database layer, better log about that
        Err(e) => {
            let msg = format!(
                "Unable to read database table jade_file_pair for uuid '{find_uuid}': {e}."
            );
            error!(msg);
            Err(DatamoveError::Critical(msg))
        }
    }
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
