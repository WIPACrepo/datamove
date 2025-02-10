// disk.rs

use chrono::{NaiveDateTime, Utc};
use log::{error, info, trace};
use sqlx::MySqlPool;
use std::collections::HashSet;

use crate::sps::jade_db::repo::disk::find_by_uuid as repo_find_by_uuid;
use crate::sps::jade_db::repo::disk::get_num_file_pairs as repo_get_num_file_pairs;
use crate::sps::jade_db::repo::disk::get_removable_files as repo_get_removable_files;
use crate::sps::jade_db::repo::disk::get_size_file_pairs as repo_get_size_file_pairs;
use crate::sps::jade_db::repo::disk::save as repo_save;
use crate::sps::jade_db::repo::disk::MySqlJadeDisk;
use crate::sps::jade_db::utils::convert_primitive_date_time_to_naive_date_time as to_naive_date_time;

pub type Error = Box<dyn core::error::Error>;
pub type Result<T> = core::result::Result<T, Error>;

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
    pub hardware_metadata: String,
    pub serial_number: String,
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
            hardware_metadata: value
                .hardware_metadata
                .expect("jade_disk.hardware_metadata IS null"),
            serial_number: "".into(), // TODO: implement this!
        }
    }
}

pub async fn close(pool: &MySqlPool, jade_disk: &JadeDisk) -> Result<u64> {
    let jade_disk_id = jade_disk.jade_disk_id;
    let disk_uuid = &jade_disk.uuid;
    trace!("jade_db::service::disk::close({jade_disk_id}:{disk_uuid})");

    // modify the disk to close it
    let now = Utc::now().naive_utc();
    let mut closed_disk = jade_disk.clone();
    closed_disk.closed = true;
    closed_disk.date_updated = now;

    // save the disk to the database
    let mysql_jade_disk: MySqlJadeDisk = jade_disk.into();
    let count = repo_save(pool, &mysql_jade_disk).await?;

    // if we were able to update the single row, log our success
    if count == 1 {
        info!("Updated row in table jade_disk for uuid '{disk_uuid}'");
        return Ok(count);
    }

    // oops, something went wrong...
    error!("Unable to update row in table jade_disk for uuid '{disk_uuid}' ({count})");
    Err("DB: update jade_disk".into())
}

pub async fn close_by_uuid(pool: &MySqlPool, find_uuid: &str) -> Result<u64> {
    let jade_disk = find_by_uuid(pool, find_uuid).await?;
    close(pool, &jade_disk).await
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
                error!("Database table jade_disk has no entry for uuid '{find_uuid}'.");
                Err(format!("Database table jade_disk has no entry for uuid '{find_uuid}'.").into())
            }
        }
        // whoops, something went wrong in the database layer, better log about that
        Err(e) => {
            error!("Unable to read database table jade_disk for uuid '{find_uuid}': {e}.");
            Err(
                format!("Unable to read database table jade_disk for uuid '{find_uuid}': {e}.")
                    .into(),
            )
        }
    }
}

pub async fn get_num_file_pairs(pool: &MySqlPool, jade_disk: &JadeDisk) -> Result<i64> {
    repo_get_num_file_pairs(pool, jade_disk.jade_disk_id).await
}

pub async fn get_removable_files(
    pool: &MySqlPool,
    loaded_disk_ids: &Vec<i64>,
    required_copies: u64,
) -> Result<HashSet<String>> {
    repo_get_removable_files(pool, loaded_disk_ids, required_copies).await
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
