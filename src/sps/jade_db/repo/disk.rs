// disk.rs

use std::collections::HashSet;
use std::time::SystemTime;

use chrono::{DateTime, NaiveDateTime, Utc};
use num_traits::cast::ToPrimitive;
use sqlx::types::time::PrimitiveDateTime;
use sqlx::{FromRow, MySql, MySqlPool, Transaction};
use time::OffsetDateTime;
use tracing::{info, trace};

use crate::sps::jade_db::repo::file_pair::MySqlJadeFilePair;
use crate::sps::jade_db::service::disk::JadeDisk;
use crate::sps::jade_db::utils::JadeDatePrimitive;

use crate::error::{DatamoveError, Result};

#[derive(Debug, FromRow)]
pub struct MySqlJadeDisk {
    pub jade_disk_id: i64,
    pub bad: Option<u8>,
    pub capacity: Option<i64>,
    pub closed: Option<u8>,
    pub copy_id: Option<i32>,
    pub date_created: Option<PrimitiveDateTime>,
    pub date_updated: Option<PrimitiveDateTime>,
    pub device_path: Option<String>,
    pub label: Option<String>,
    pub on_hold: Option<u8>,
    pub uuid: Option<String>,
    pub version: Option<i64>,
    pub jade_host_id: Option<i64>,
    pub disk_archive_uuid: Option<String>,
    pub serial_number: Option<String>,
    pub hardware_metadata: Option<String>,
}

impl From<&JadeDisk> for MySqlJadeDisk {
    fn from(value: &JadeDisk) -> Self {
        fn convert_date(ndt: NaiveDateTime) -> PrimitiveDateTime {
            let jade_date: JadeDatePrimitive = ndt.into();
            jade_date.into()
        }

        MySqlJadeDisk {
            jade_disk_id: value.jade_disk_id,
            bad: if value.bad { Some(0x01) } else { Some(0x00) },
            capacity: Some(value.capacity),
            closed: if value.closed { Some(0x01) } else { Some(0x00) },
            copy_id: Some(value.copy_id),
            date_created: Some(convert_date(value.date_created)),
            date_updated: Some(convert_date(value.date_updated)),
            device_path: Some(value.device_path.clone()),
            label: Some(value.label.clone()),
            on_hold: if value.on_hold {
                Some(0x01)
            } else {
                Some(0x00)
            },
            uuid: Some(value.uuid.clone()),
            version: Some(value.version),
            jade_host_id: Some(value.jade_host_id),
            disk_archive_uuid: Some(value.disk_archive_uuid.clone()),
            serial_number: Some(value.serial_number.clone()),
            hardware_metadata: Some(value.hardware_metadata.clone()),
        }
    }
}

pub async fn add_file_pair(
    pool: &MySqlPool,
    jade_disk_id: i64,
    jade_file_pair_id: i64,
) -> Result<()> {
    info!(
        "add_file_pair(): jade_disk_id={} jade_file_pair_id={}",
        jade_disk_id, jade_file_pair_id
    );
    // create a transaction to update the disk contents atomically
    let mut tx: Transaction<MySql> = pool.begin().await?;
    // check if the mapping already exists
    let exists = sqlx::query_scalar!(
        r#"
        SELECT 1
        FROM jade_map_disk_to_file_pair
        WHERE jade_disk_id = ?
          AND jade_file_pair_id = ?
        "#,
        jade_disk_id,
        jade_file_pair_id
    )
    .fetch_optional(&mut *tx)
    .await?;
    // if the mapping already exists, bail out
    if exists.is_some() {
        tx.rollback().await?;
        return Ok(());
    }
    // determine the next order value (starting from 0)
    let next_order = sqlx::query_scalar!(
        r#"
        SELECT COALESCE(MAX(jade_file_pair_order) + 1, 0)
        FROM jade_map_disk_to_file_pair
        WHERE jade_disk_id = ?
        "#,
        jade_disk_id
    )
    .fetch_one(&mut *tx)
    .await?;
    // insert the new mapping
    let _result = sqlx::query!(
        r#"
        INSERT INTO jade_map_disk_to_file_pair (
            jade_disk_id,
            jade_file_pair_id,
            jade_file_pair_order
        ) VALUES (
            ?,
            ?,
            ?
        )
        "#,
        jade_disk_id,
        jade_file_pair_id,
        next_order
    )
    .execute(&mut *tx)
    .await?;
    // commit the transaction to the database
    tx.commit().await?;
    // tell the caller that we succeeded
    trace!(
        "Done -- add_file_pair(): jade_disk_id={} jade_file_pair_id={}",
        jade_disk_id,
        jade_file_pair_id
    );
    Ok(())
}

pub async fn count_file_pair_copies(pool: &MySqlPool, jade_file_pair_id: i64) -> Result<i64> {
    // query the database to determine how many good copies of a file exist on archival disks
    let copy_count = sqlx::query_scalar!(
        r#"
        SELECT COUNT(jmdtfp.jade_disk_id) as copy_count
        FROM jade_map_disk_to_file_pair AS jmdtfp
        LEFT JOIN jade_disk AS jd ON jd.jade_disk_id = jmdtfp.jade_disk_id
        WHERE jmdtfp.jade_file_pair_id = ?
          AND jd.bad = false
          AND jd.closed = true
          AND jd.on_hold = false
        "#,
        jade_file_pair_id,
    )
    .fetch_one(pool)
    .await?;
    // return that result to the caller
    Ok(copy_count)
}

pub async fn create(pool: &MySqlPool, jade_disk: &MySqlJadeDisk) -> Result<u64> {
    let jade_disk_id = sqlx::query!(
        r#"
            INSERT INTO jade_disk (
                bad,
                capacity,
                closed,
                copy_id,
                date_created,
                date_updated,
                device_path,
                label,
                on_hold,
                uuid,
                version,
                jade_host_id,
                disk_archive_uuid,
                serial_number,
                hardware_metadata
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
                ?
            )
        "#,
        jade_disk.bad,
        jade_disk.capacity,
        jade_disk.closed,
        jade_disk.copy_id,
        jade_disk.date_created,
        jade_disk.date_updated,
        jade_disk.device_path,
        jade_disk.label,
        jade_disk.on_hold,
        jade_disk.uuid,
        jade_disk.version,
        jade_disk.jade_host_id,
        jade_disk.disk_archive_uuid,
        jade_disk.serial_number,
        jade_disk.hardware_metadata
    )
    .execute(pool)
    .await?
    .last_insert_id();

    Ok(jade_disk_id)
}

pub async fn find_archived_file_pair_ids(pool: &MySqlPool, jade_disk_id: i64) -> Result<Vec<i64>> {
    let jade_file_pair_ids = sqlx::query_scalar!(
        r#"
            SELECT jade_file_pair_id
            FROM jade_map_disk_to_file_pair
            WHERE jade_disk_id = ?
            ORDER BY jade_file_pair_order
        "#,
        jade_disk_id,
    )
    .fetch_all(pool)
    .await?;

    Ok(jade_file_pair_ids)
}

pub async fn find_archived_file_pair_uuids(
    pool: &MySqlPool,
    jade_disk_id: i64,
) -> Result<Vec<String>> {
    let jade_file_pair_uuids = sqlx::query_scalar!(
        r#"
            SELECT jfp.jade_file_pair_uuid as jade_fi
            FROM jade_map_disk_to_file_pair AS jmdtfp
            LEFT JOIN jade_file_pair AS jfp
            ON jfp.jade_file_pair_id = jmdtfp.jade_file_pair_id
            WHERE jade_disk_id = ?
            ORDER BY jade_file_pair_order
        "#,
        jade_disk_id,
    )
    .fetch_all(pool)
    .await?;

    // remove all the NULL values (note: column is always populated in practice anyway)
    let jade_file_pair_uuids: Vec<String> = jade_file_pair_uuids.into_iter().flatten().collect();

    Ok(jade_file_pair_uuids)
}

pub async fn find_by_id(pool: &MySqlPool, jade_disk_id: i64) -> Result<Option<MySqlJadeDisk>> {
    let result = sqlx::query_as!(
        MySqlJadeDisk,
        r#"SELECT * FROM jade_disk WHERE jade_disk_id = ?"#,
        jade_disk_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(result)
}

pub async fn find_by_uuid(pool: &MySqlPool, uuid: &str) -> Result<Option<MySqlJadeDisk>> {
    let result = sqlx::query_as!(
        MySqlJadeDisk,
        r#"SELECT * FROM jade_disk WHERE uuid = ?"#,
        uuid
    )
    .fetch_optional(pool)
    .await?;

    Ok(result)
}

/// find if a file pair is archived to a good archive X, copy Y disk
pub async fn find_file_pair(
    pool: &MySqlPool,
    jade_host_id: i64,
    disk_archive_uuid: &str,
    copy_id: u64,
    jade_file_pair_id: i64,
) -> Result<Option<MySqlJadeFilePair>> {
    let result = sqlx::query_as!(
        MySqlJadeFilePair,
        r#"
            SELECT jfp.*
            FROM jade_file_pair AS jfp
            LEFT JOIN jade_map_disk_to_file_pair AS jmdtfp
            ON jfp.jade_file_pair_id = jmdtfp.jade_file_pair_id
            LEFT JOIN jade_disk AS jd
            ON jd.jade_disk_id = jmdtfp.jade_disk_id
            WHERE jd.bad = false
            AND jfp.jade_file_pair_id = ?
            AND jd.jade_host_id = ?
            AND jd.disk_archive_uuid = ?
            AND jd.copy_id = ?
        "#,
        jade_file_pair_id,
        jade_host_id,
        disk_archive_uuid,
        copy_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(result)
}

/// find an open disk for archive X, copy Y
pub async fn find_open(
    pool: &MySqlPool,
    jade_host_id: i64,
    disk_archive_uuid: &str,
    copy_id: i32,
) -> Result<Option<MySqlJadeDisk>> {
    trace!(
        "find_open() Host:{} DiskArchive:{} Copy:{}",
        jade_host_id,
        disk_archive_uuid,
        copy_id
    );
    trace!("Database connections in use: {}", pool.size());
    let result = sqlx::query_as!(
        MySqlJadeDisk,
        r#"
            SELECT *
            FROM jade_disk
            WHERE bad = false
              AND closed = false
              AND copy_id = ?
              AND jade_host_id = ?
              AND disk_archive_uuid = ?
        "#,
        copy_id,
        jade_host_id,
        disk_archive_uuid
    )
    .fetch_optional(pool)
    .await?;

    // tell the caller about the open disk we found
    trace!(
        "find_open() Host:{} DiskArchive:{} Copy:{} -- Found:{}",
        jade_host_id,
        disk_archive_uuid,
        copy_id,
        result.is_some()
    );
    trace!("Database connections in use: {}", pool.size());
    Ok(result)
}

pub async fn get_num_file_pairs(pool: &MySqlPool, jade_disk_id: i64) -> Result<i64> {
    let result = sqlx::query!(
        r#"
            select count(distinct jmdtfp.jade_file_pair_id) as num_files
            from jade_map_disk_to_file_pair as jmdtfp
            where jmdtfp.jade_disk_id = ?
        "#,
        jade_disk_id
    )
    .fetch_one(pool)
    .await?;

    Ok(result.num_files)
}

// TODO: Remove this
// pub async fn get_removable_files(
//     pool: &MySqlPool,
//     loaded_disk_ids: &Vec<i64>,
//     required_copies: u64,
// ) -> Result<HashSet<String>> {
//     // if no disks are loaded, return an empty set
//     if loaded_disk_ids.is_empty() {
//         return Ok(HashSet::new());
//     }
//     trace!("loaded_disk_ids: {loaded_disk_ids:?}");
//     // create placeholders; 4 ids = ?,?,?,?
//     let placeholders = loaded_disk_ids
//         .iter()
//         .map(|_| "?")
//         .collect::<Vec<_>>()
//         .join(", ");
//     // create the query text to find files that are sufficiently archived
//     let query_string = format!(
//         "WITH good_disks AS (
//             SELECT jade_disk_id
//             FROM jade_disk
//             WHERE closed = 1 AND bad = 0 AND on_hold = 0
//             AND jade_disk_id IN ({})
//         ),
//         file_copy_counts AS (
//             SELECT jfp.jade_file_pair_uuid, COUNT(*) AS copy_count
//             FROM jade_map_disk_to_file_pair jmdfp
//             JOIN good_disks gd ON jmdfp.jade_disk_id = gd.jade_disk_id
//             JOIN jade_file_pair jfp ON jmdfp.jade_file_pair_id = jfp.jade_file_pair_id
//             GROUP BY jfp.jade_file_pair_uuid
//         )
//         SELECT jade_file_pair_uuid
//         FROM file_copy_counts
//         WHERE copy_count >= ?;",
//         placeholders
//     );
//     // create the query object to run against the database
//     let mut query = sqlx::query_scalar(&query_string);
//     // bind the disk ids to the placeholders we created dynamically
//     for id in loaded_disk_ids {
//         query = query.bind(id);
//     }
//     // bind the required number of copies to the final placeholder
//     query = query.bind(required_copies);
//     // get the list of jade_file_pair_uuid values
//     let jade_file_pair_uuids: Vec<String> = query.fetch_all(pool).await?;
//     // send the list of UUIDs back to the caller in a HashSet
//     Ok(jade_file_pair_uuids.into_iter().collect())
// }

pub async fn get_removable_files(
    pool: &MySqlPool,
    cache_date: SystemTime,
    required_copies: u64,
) -> Result<HashSet<String>> {
    // determine the date of the earliest file in the cache
    let earliest_file: DateTime<Utc> = cache_date.into();
    // query the database to determine the files ready to be deleted
    let jade_file_pair_uuids: Vec<String> = sqlx::query_scalar(
        "WITH good_disks AS (
            SELECT jade_disk_id
            FROM jade_disk
            WHERE closed = 1
              AND bad = 0
              AND on_hold = 0
              AND date_created >= DATE_SUB(?, INTERVAL 30 DAY)
        ),
        file_copy_counts AS (
            SELECT jfp.jade_file_pair_uuid, COUNT(*) AS copy_count
            FROM jade_map_disk_to_file_pair jmdfp
            JOIN good_disks gd ON jmdfp.jade_disk_id = gd.jade_disk_id
            JOIN jade_file_pair jfp ON jmdfp.jade_file_pair_id = jfp.jade_file_pair_id
            GROUP BY jfp.jade_file_pair_uuid
        )
        SELECT jade_file_pair_uuid
        FROM file_copy_counts
        WHERE copy_count >= ?",
    )
    .bind(earliest_file)
    .bind(required_copies)
    .fetch_all(pool)
    .await?;
    // send the list of UUIDs back to the caller in a HashSet
    Ok(jade_file_pair_uuids.into_iter().collect())
}

pub async fn get_serial_number_age_in_secs(
    pool: &MySqlPool,
    serial_number: &str,
) -> Result<Option<u32>> {
    let result = sqlx::query!(
        r#"
            select max(date_created) as latest_use
            from jade_disk as jd
            where jd.serial_number = ?
        "#,
        serial_number
    )
    .fetch_one(pool)
    .await?;

    // did we get anything back from the database?
    match result.latest_use {
        // No? Okay, return None
        None => Ok(None),
        // Yes? Okay, convert the result to age in seconds
        Some(latest_use) => {
            // determine how long ago it was in seconds
            let now = OffsetDateTime::now_utc();
            let past_offset = latest_use.assume_utc();
            let duration = now - past_offset;
            let seconds = duration.whole_seconds();
            // if somebody went 88 miles an hour... https://www.youtube.com/watch?v=JFT7hNhop7w
            if seconds < 0 {
                let msg = format!(
                    "Serial Number:'{}' was used in the future!? (Age:{}s)",
                    serial_number, seconds
                );
                return Err(DatamoveError::Critical(msg));
            }
            // otherwise, turn it into a u32
            Ok(Some(seconds as u32))
        }
    }
}

pub async fn get_size_file_pairs(pool: &MySqlPool, jade_disk_id: i64) -> Result<i64> {
    let result = sqlx::query!(
        r#"
            select sum(jfp.archive_size) as total_bytes
            from jade_disk as jd
            left join jade_map_disk_to_file_pair as jmdtfp
                on jmdtfp.jade_disk_id = jd.jade_disk_id
            left join jade_file_pair as jfp
                on jfp.jade_file_pair_id = jmdtfp.jade_file_pair_id
            where jd.jade_disk_id = ?
        "#,
        jade_disk_id
    )
    .fetch_one(pool)
    .await?;

    let total_bytes = result
        .total_bytes
        .unwrap_or_else(|| {
            panic!("get_size_file_pairs(jade_disk_id = {jade_disk_id}) query returned NULL result")
        })
        .to_i64()
        .expect("unable to convert BigDecimal to i64");

    Ok(total_bytes)
}

pub async fn save(pool: &MySqlPool, jade_disk: &MySqlJadeDisk) -> Result<u64> {
    trace!(
        "save(): jade_disk_id={} closed={:?}",
        jade_disk.jade_disk_id,
        jade_disk.closed
    );
    // create a transaction to update the disk atomically
    let mut tx: Transaction<MySql> = pool.begin().await?;
    // run the update query
    let rows_affected = sqlx::query!(
        r#"
            UPDATE jade_disk
            SET bad = ?,
                capacity = ?,
                closed = ?,
                copy_id = ?,
                date_created = ?,
                date_updated = ?,
                device_path = ?,
                label = ?,
                on_hold = ?,
                uuid = ?,
                version = ?,
                jade_host_id = ?,
                disk_archive_uuid = ?,
                serial_number = ?,
                hardware_metadata = ?
            WHERE jade_disk_id = ?
        "#,
        jade_disk.bad,
        jade_disk.capacity,
        jade_disk.closed,
        jade_disk.copy_id,
        jade_disk.date_created,
        jade_disk.date_updated,
        jade_disk.device_path,
        jade_disk.label,
        jade_disk.on_hold,
        jade_disk.uuid,
        jade_disk.version,
        jade_disk.jade_host_id,
        jade_disk.disk_archive_uuid,
        jade_disk.serial_number,
        jade_disk.hardware_metadata,
        jade_disk.jade_disk_id
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();
    // commit the transaction to the database
    tx.commit().await?;
    // tell the caller that we succeeded
    Ok(rows_affected)
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
