// disk_label.rs

use chrono::Datelike;
use chrono::{NaiveDateTime, Utc};
use log::error;
use sqlx::MySqlPool;

use crate::config::DiskArchive;
use crate::sps::jade_db::repo::disk_label::find_by_id as repo_find_by_id;
use crate::sps::jade_db::repo::disk_label::get_next_label as repo_get_next_label;
use crate::sps::jade_db::repo::disk_label::MySqlJadeDiskLabel;
use crate::sps::jade_db::utils::convert_primitive_date_time_to_naive_date_time as to_naive_date_time;

pub type Error = Box<dyn core::error::Error>;
pub type Result<T> = core::result::Result<T, Error>;

#[derive(Clone)]
pub struct JadeDiskLabel {
    pub jade_disk_label_id: i64,
    pub version: i64,
    pub date_created: NaiveDateTime,
    pub date_updated: NaiveDateTime,
    pub disk_archive_uuid: String,
    pub copy_id: i32,
    pub disk_archive_year: i32,
    pub disk_archive_sequence: i32,
}

impl From<MySqlJadeDiskLabel> for JadeDiskLabel {
    fn from(value: MySqlJadeDiskLabel) -> Self {
        JadeDiskLabel {
            jade_disk_label_id: value.jade_disk_label_id,
            version: value.version.expect("jade_disk_label.version IS null"),
            date_created: to_naive_date_time(
                &value
                    .date_created
                    .expect("jade_disk_label.date_created IS null"),
            ),
            date_updated: to_naive_date_time(
                &value
                    .date_updated
                    .expect("jade_disk_label.date_updated IS null"),
            ),
            disk_archive_uuid: value
                .disk_archive_uuid
                .expect("jade_disk_label.disk_archive_uuid IS null"),
            copy_id: value.copy_id.expect("jade_disk_label.copy_id IS null"),
            disk_archive_year: value
                .disk_archive_year
                .expect("jade_disk_label.disk_archive_year IS null"),
            disk_archive_sequence: value
                .disk_archive_sequence
                .expect("jade_disk_label.disk_archive_sequence IS null"),
        }
    }
}

//---------------------------------------------------------------------------------------------------------------------
//---------------------------------------------------------------------------------------------------------------------
//---------------------------------------------------------------------------------------------------------------------

pub async fn find_by_id(pool: &MySqlPool, jade_disk_label_id: i64) -> Result<JadeDiskLabel> {
    // try to locate the disk label by id in the database
    match repo_find_by_id(pool, jade_disk_label_id).await {
        Ok(disk_label) => {
            if let Some(disk_label) = disk_label {
                let jade_disk_label: JadeDiskLabel = disk_label.into();
                Ok(jade_disk_label)
            } else {
                let msg = format!(
                    "Database table jade_disk_label has no entry for id '{jade_disk_label_id}'."
                );
                error!("{msg}");
                Err(msg.into())
            }
        }
        Err(e) => {
            let msg = format!(
                "Unable to read database table jade_disk_label for id '{jade_disk_label_id}': {e}."
            );
            error!("{msg}");
            Err(msg.into())
        }
    }
}

pub async fn get_next_label(
    pool: &MySqlPool,
    disk_archive: &DiskArchive,
    copy_id: u64,
) -> Result<String> {
    let disk_archive_uuid = &disk_archive.uuid;
    let disk_archive_year = Utc::now().year() as u64;
    let sequence_number =
        repo_get_next_label(pool, disk_archive_uuid, copy_id, disk_archive_year).await?;
    let name = &disk_archive.short_name;
    Ok(format!(
        "{}_{}_{}_{:04}",
        name, copy_id, disk_archive_year, sequence_number
    ))
}

//---------------------------------------------------------------------------------------------------------------------
//---------------------------------------------------------------------------------------------------------------------
//---------------------------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    // use super::*;

    #[test]
    fn test_always_succeed() {
        assert!(true);
    }
}
