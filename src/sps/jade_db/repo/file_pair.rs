// file_pair.rs

use chrono::NaiveDateTime;
use sqlx::types::time::PrimitiveDateTime;
use sqlx::{FromRow, MySqlPool};

use crate::sps::jade_db::service::file_pair::JadeFilePair;
use crate::sps::jade_db::utils::JadeDatePrimitive;

use crate::error::Result;

#[derive(Debug, FromRow)]
pub struct MySqlJadeFilePair {
    pub jade_file_pair_id: i64,
    pub archive_checksum: Option<String>,
    pub archive_file: Option<String>,
    pub archive_size: Option<i64>,
    pub binary_file: Option<String>,
    pub binary_size: Option<i64>,
    pub date_archived: Option<PrimitiveDateTime>,
    pub date_created: Option<PrimitiveDateTime>,
    pub date_fetched: Option<PrimitiveDateTime>,
    pub date_processed: Option<PrimitiveDateTime>,
    pub date_updated: Option<PrimitiveDateTime>,
    pub date_verified: Option<PrimitiveDateTime>,
    pub fetch_checksum: Option<String>,
    pub fingerprint: Option<String>,
    pub ingest_checksum: Option<i64>,
    pub metadata_file: Option<String>,
    pub origin_checksum: Option<String>,
    pub date_modified_origin: Option<PrimitiveDateTime>,
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

impl From<JadeFilePair> for MySqlJadeFilePair {
    fn from(value: JadeFilePair) -> Self {
        fn convert_date(ndt: Option<NaiveDateTime>) -> Option<PrimitiveDateTime> {
            match ndt {
                Some(ndt) => {
                    let jade_date: JadeDatePrimitive = ndt.into();
                    let pdt: PrimitiveDateTime = jade_date.into();
                    Some(pdt)
                }
                None => None,
            }
        }

        MySqlJadeFilePair {
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

pub async fn create(pool: &MySqlPool, jade_file_pair: &MySqlJadeFilePair) -> Result<u64> {
    let jade_file_pair_id = sqlx::query!(
        r#"
            INSERT INTO jade_file_pair (
                archive_checksum,
                archive_file,
                archive_size,
                binary_file,
                binary_size,
                date_archived,
                date_created,
                date_fetched,
                date_processed,
                date_updated,
                date_verified,
                fetch_checksum,
                fingerprint,
                ingest_checksum,
                metadata_file,
                origin_checksum,
                date_modified_origin,
                semaphore_file,
                version,
                archived_by_host_id,
                jade_data_stream_id,
                fetched_by_host_id,
                processed_by_host_id,
                verified_by_host_id,
                jade_data_stream_uuid,
                jade_file_pair_uuid,
                priority_group,
                data_warehouse_path
            ) VALUES (
                ?,
                ?,
                ?,
                ?,
                ?,
                ?,
                ?,
                ?,
                ?,
                ?,
                ?,
                ?,
                ?,
                ?,
                ?,
                ?,
                ?,
                ?,
                ?,
                ?,
                ?,
                ?,
                ?,
                ?,
                ?,
                ?,
                ?,
                ?
            )
        "#,
        jade_file_pair.archive_checksum,
        jade_file_pair.archive_file,
        jade_file_pair.archive_size,
        jade_file_pair.binary_file,
        jade_file_pair.binary_size,
        jade_file_pair.date_archived,
        jade_file_pair.date_created,
        jade_file_pair.date_fetched,
        jade_file_pair.date_processed,
        jade_file_pair.date_updated,
        jade_file_pair.date_verified,
        jade_file_pair.fetch_checksum,
        jade_file_pair.fingerprint,
        jade_file_pair.ingest_checksum,
        jade_file_pair.metadata_file,
        jade_file_pair.origin_checksum,
        jade_file_pair.date_modified_origin,
        jade_file_pair.semaphore_file,
        jade_file_pair.version,
        jade_file_pair.archived_by_host_id,
        jade_file_pair.jade_data_stream_id,
        jade_file_pair.fetched_by_host_id,
        jade_file_pair.processed_by_host_id,
        jade_file_pair.verified_by_host_id,
        jade_file_pair.jade_data_stream_uuid,
        jade_file_pair.jade_file_pair_uuid,
        jade_file_pair.priority_group,
        jade_file_pair.data_warehouse_path,
    )
    .execute(pool)
    .await?
    .last_insert_id();

    Ok(jade_file_pair_id)
}

pub async fn find_by_id(
    pool: &MySqlPool,
    jade_file_pair_id: i64,
) -> Result<Option<MySqlJadeFilePair>> {
    let result = sqlx::query_as!(
        MySqlJadeFilePair,
        r#"SELECT * FROM jade_file_pair WHERE jade_file_pair_id = ?"#,
        jade_file_pair_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(result)
}

pub async fn find_by_uuid(
    pool: &MySqlPool,
    jade_file_pair_uuid: &str,
) -> Result<Option<MySqlJadeFilePair>> {
    let result = sqlx::query_as!(
        MySqlJadeFilePair,
        r#"SELECT * FROM jade_file_pair WHERE jade_file_pair_uuid = ?"#,
        jade_file_pair_uuid
    )
    .fetch_optional(pool)
    .await?;

    Ok(result)
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
