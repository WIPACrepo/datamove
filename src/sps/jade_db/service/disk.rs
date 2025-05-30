// disk.rs

use std::collections::HashSet;
use std::time::SystemTime;

use chrono::{NaiveDateTime, Utc};
use sqlx::MySqlPool;
use tracing::{error, info, trace};

use crate::config::DiskArchive;
use crate::sps::jade_db::repo::disk::add_file_pair as repo_add_file_pair;
use crate::sps::jade_db::repo::disk::count_file_pair_copies as repo_count_file_pair_copies;
use crate::sps::jade_db::repo::disk::create as repo_create;
use crate::sps::jade_db::repo::disk::find_archived_file_pair_ids as repo_find_archived_file_pair_ids;
use crate::sps::jade_db::repo::disk::find_archived_file_pair_uuids as repo_find_archived_file_pair_uuids;
use crate::sps::jade_db::repo::disk::find_by_uuid as repo_find_by_uuid;
use crate::sps::jade_db::repo::disk::find_file_pair as repo_find_file_pair;
use crate::sps::jade_db::repo::disk::find_open as repo_find_open;
use crate::sps::jade_db::repo::disk::get_num_file_pairs as repo_get_num_file_pairs;
use crate::sps::jade_db::repo::disk::get_removable_files as repo_get_removable_files;
use crate::sps::jade_db::repo::disk::get_serial_number_age_in_secs as repo_get_serial_number_age_in_secs;
use crate::sps::jade_db::repo::disk::get_size_file_pairs as repo_get_size_file_pairs;
use crate::sps::jade_db::repo::disk::save as repo_save;
use crate::sps::jade_db::repo::disk::MySqlJadeDisk;
use crate::sps::jade_db::service::file_pair::JadeFilePair;
use crate::sps::jade_db::utils::convert_primitive_date_time_to_naive_date_time as to_naive_date_time;
use crate::sps::process::disk_archiver::DiskArchiver;

use crate::error::{DatamoveError, Result};

pub const ONE_YEAR_IN_SECONDS: u32 = 31_536_000;
pub const TEN_YEARS_IN_SECONDS: u32 = 315_360_000;

#[derive(Clone)]
pub struct JadeDisk {
    pub jade_disk_id: i64,
    pub bad: bool,
    pub capacity: i64,
    pub closed: bool,
    pub copy_id: i32,
    pub date_created: NaiveDateTime,
    pub date_updated: NaiveDateTime,
    pub device_path: String,
    pub label: String,
    pub on_hold: bool,
    pub uuid: String,
    pub version: i64,
    pub jade_host_id: i64,
    pub disk_archive_uuid: String,
    pub serial_number: String,
    pub hardware_metadata: String,
}

impl From<MySqlJadeDisk> for JadeDisk {
    fn from(value: MySqlJadeDisk) -> Self {
        JadeDisk {
            jade_disk_id: value.jade_disk_id,
            bad: value.bad.expect("jade_disk.bad IS null") & 0x1 == 0x1,
            capacity: value.capacity.expect("jade_disk.capacity IS null"),
            closed: value.closed.expect("jade_disk.closed IS null") & 0x1 == 0x1,
            copy_id: value.copy_id.expect("jade_disk.copy_id IS null"),
            date_created: to_naive_date_time(
                &value.date_created.expect("jade_disk.date_created IS null"),
            ),
            date_updated: to_naive_date_time(
                &value.date_updated.expect("jade_disk.date_updated IS null"),
            ),
            device_path: value.device_path.expect("jade_disk.device_path IS null"),
            label: value.label.expect("jade_disk.label IS null"),
            on_hold: value.on_hold.expect("jade_disk.on_hold IS null") & 0x1 == 0x1,
            uuid: value.uuid.expect("jade_disk.uuid IS null"),
            version: value.version.expect("jade_disk.version IS null"),
            jade_host_id: value.jade_host_id.expect("jade_disk.jade_host_id IS null"),
            disk_archive_uuid: value
                .disk_archive_uuid
                .expect("jade_disk.disk_archive_uuid IS null"),
            serial_number: value
                .serial_number
                .expect("jade_disk.serial_number IS null"),
            hardware_metadata: value
                .hardware_metadata
                .expect("jade_disk.hardware_metadata IS null"),
        }
    }
}

pub async fn add_file_pair(
    pool: &MySqlPool,
    jade_disk: &JadeDisk,
    file_pair: &JadeFilePair,
) -> Result<()> {
    info!(
        "add_file_pair(): jade_disk_id={} jade_file_pair_id={}",
        jade_disk.jade_disk_id, file_pair.jade_file_pair_id
    );
    repo_add_file_pair(pool, jade_disk.jade_disk_id, file_pair.jade_file_pair_id).await
}

pub async fn close(pool: &MySqlPool, jade_disk: &JadeDisk) -> Result<u64> {
    let jade_disk_id = jade_disk.jade_disk_id;
    let disk_uuid = &jade_disk.uuid;
    info!(
        "close(): jade_disk_id={} disk_uuid={}",
        jade_disk_id, disk_uuid
    );

    // modify the disk to close it
    let now = Utc::now().naive_utc();
    let mut closed_disk = jade_disk.clone();
    closed_disk.closed = true;
    closed_disk.date_updated = now;

    // save the disk to the database
    let mysql_jade_disk: MySqlJadeDisk = (&closed_disk).into();
    let count = repo_save(pool, &mysql_jade_disk).await?;

    // if we were able to update the single row, log our success
    if count == 1 {
        info!("Updated row in table jade_disk for uuid '{disk_uuid}'");
        return Ok(count);
    }

    // oops, something went wrong...
    let msg = format!("Unable to update row in table jade_disk for uuid '{disk_uuid}' ({count})");
    error!(msg);
    Err(DatamoveError::Critical(msg))
}

pub async fn close_by_uuid(pool: &MySqlPool, find_uuid: &str) -> Result<u64> {
    let jade_disk = find_by_uuid(pool, find_uuid).await?;
    close(pool, &jade_disk).await
}

pub async fn count_file_pair_copies(
    pool: &MySqlPool,
    jade_file_pair: &JadeFilePair,
) -> Result<i64> {
    repo_count_file_pair_copies(pool, jade_file_pair.jade_file_pair_id).await
}

pub async fn create(pool: &MySqlPool, jade_disk: &JadeDisk) -> Result<u64> {
    let mysql_jade_disk: MySqlJadeDisk = jade_disk.into();
    repo_create(pool, &mysql_jade_disk).await
}

pub async fn find_archived_file_pair_ids(pool: &MySqlPool, disk: &JadeDisk) -> Result<Vec<i64>> {
    repo_find_archived_file_pair_ids(pool, disk.jade_disk_id).await
}

pub async fn find_archived_file_pair_uuids(
    pool: &MySqlPool,
    disk: &JadeDisk,
) -> Result<Vec<String>> {
    repo_find_archived_file_pair_uuids(pool, disk.jade_disk_id).await
}

pub async fn find_by_uuid(pool: &MySqlPool, find_uuid: &str) -> Result<JadeDisk> {
    // try to locate the disk by uuid in the database
    match repo_find_by_uuid(pool, find_uuid).await {
        // if we got a result back from the database
        Ok(disk) => {
            if let Some(disk) = disk {
                // convert it to a service layer JadeDisk and return it to the caller
                let jade_disk: JadeDisk = disk.into();
                Ok(jade_disk)
            } else {
                // otherwise log the missing disk as an error and return an Err Result
                let msg = format!("Database table jade_disk has no entry for uuid '{find_uuid}'.");
                error!(msg);
                Err(DatamoveError::Critical(msg))
            }
        }
        // whoops, something went wrong in the database layer, better log about that
        Err(e) => {
            let msg =
                format!("Unable to read database table jade_disk for uuid '{find_uuid}': {e}.");
            error!(msg);
            Err(DatamoveError::Critical(msg))
        }
    }
}

/// find a file pair if stored on a good archive X, copy Y disk
pub async fn find_file_pair(
    pool: &MySqlPool,
    disk_archiver: &DiskArchiver,
    disk_archive: &DiskArchive,
    copy_id: u64,
    jade_file_pair: &JadeFilePair,
) -> Result<Option<JadeFilePair>> {
    let jade_host_id = disk_archiver.host.jade_host_id;
    let jade_file_pair_id = jade_file_pair.jade_file_pair_id;
    let disk_archive_uuid = &disk_archive.uuid;

    match repo_find_file_pair(
        pool,
        jade_host_id,
        disk_archive_uuid,
        copy_id,
        jade_file_pair_id,
    )
    .await
    {
        Ok(file_pair) => {
            if let Some(file_pair) = file_pair {
                // convert it to a service layer JadeFilePair and return it to the caller
                let jade_file_pair: JadeFilePair = file_pair.into();
                Ok(Some(jade_file_pair))
            } else {
                // otherwise return an Ok(None) result
                Ok(None)
            }
        }
        // whoops, something went wrong in the database layer, better log about that
        Err(e) => {
            let msg = format!(
                "Database error finding file pair:{} on Host:{} Archive:{} Copy:{}\nError was: {e}",
                jade_file_pair_id, &disk_archiver.host.host_name, &disk_archive.name, copy_id,
            );
            error!(msg);
            Err(DatamoveError::Critical(msg))
        }
    }
}

/// find an open disk for archive X, copy Y
pub async fn find_open(
    pool: &MySqlPool,
    disk_archiver: &DiskArchiver,
    disk_archive: &DiskArchive,
    copy_id: u64,
) -> Result<Option<JadeDisk>> {
    trace!("find_open() Copy:{}", copy_id);
    // get the information we need for the query
    let jade_host_id = disk_archiver.host.jade_host_id;
    let disk_archive_uuid = &disk_archive.uuid;
    let copy_id = copy_id as i32;
    // try to locate the disk in the database
    trace!("find_open(Copy:{}) -- 1 -- querying DB", copy_id);
    match repo_find_open(pool, jade_host_id, disk_archive_uuid, copy_id).await {
        // if we got a result back from the database
        Ok(disk) => {
            trace!("find_open(Copy:{}) -- 2(Ok) -- successful result", copy_id);
            if let Some(disk) = disk {
                trace!("find_open(Copy:{}) -- 2(Ok).1 -- some result", copy_id);
                // convert it to a service layer JadeDisk and return it to the caller
                let jade_disk: JadeDisk = disk.into();
                trace!(
                    "find_open(Copy:{}) -- 2(Ok).1.1 -- returning result",
                    copy_id
                );
                Ok(Some(jade_disk))
            } else {
                trace!("find_open(Copy:{}) -- 2(Ok).2 -- None result", copy_id);
                // otherwise log the not found and return an Ok(None) result
                let msg = format!(
                    "Database table jade_disk has no open disk for Host:{} Archive:{} Copy:{}",
                    &disk_archiver.host.host_name, &disk_archive.name, copy_id,
                );
                info!("{msg}");
                trace!("find_open(Copy:{}) -- 2(Ok).2.1 -- returning None", copy_id);
                Ok(None)
            }
        }
        // whoops, something went wrong in the database layer, better log about that
        Err(e) => {
            trace!("find_open(Copy:{}) -- 2(Err) -- error result", copy_id);
            let msg = format!(
                "Unable to read database table jade_disk for Host:{} Archive:{} Copy:{}\nError was: {e}",
                &disk_archiver.host.host_name,
                &disk_archive.name,
                copy_id,
            );
            error!(msg);
            Err(DatamoveError::Critical(msg))
        }
    }
}

pub async fn get_num_file_pairs(pool: &MySqlPool, jade_disk: &JadeDisk) -> Result<i64> {
    repo_get_num_file_pairs(pool, jade_disk.jade_disk_id).await
}

// pub async fn get_removable_files(
//     pool: &MySqlPool,
//     loaded_disk_ids: &Vec<i64>,
//     required_copies: u64,
// ) -> Result<HashSet<String>> {
//     repo_get_removable_files(pool, loaded_disk_ids, required_copies).await
// }

pub async fn get_removable_files(
    pool: &MySqlPool,
    cache_date: SystemTime,
    required_copies: u64,
) -> Result<HashSet<String>> {
    repo_get_removable_files(pool, cache_date, required_copies).await
}

pub async fn get_serial_number_age_in_secs(pool: &MySqlPool, serial_number: &str) -> Result<u32> {
    let serial_number_age = repo_get_serial_number_age_in_secs(pool, serial_number).await?;
    if let Some(secs) = serial_number_age {
        info!("Serial number '{serial_number}' is {secs} seconds old.");
        return Ok(secs);
    }
    info!("Serial number '{serial_number}' has not been used before.");
    Ok(TEN_YEARS_IN_SECONDS)
}

pub async fn get_size_file_pairs(pool: &MySqlPool, jade_disk: &JadeDisk) -> Result<i64> {
    repo_get_size_file_pairs(pool, jade_disk.jade_disk_id).await
}

pub async fn save(pool: &MySqlPool, jade_disk: &JadeDisk) -> Result<u64> {
    let mysql_jade_disk: MySqlJadeDisk = jade_disk.into();
    repo_save(pool, &mysql_jade_disk).await
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
