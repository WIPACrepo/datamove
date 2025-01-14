// disk.rs

use chrono::NaiveDateTime;
use num_traits::cast::ToPrimitive;
use sqlx::types::time::PrimitiveDateTime;
use sqlx::{FromRow, MySqlPool};

use crate::sps::jade_db::service::disk::JadeDisk;
use crate::sps::jade_db::utils::JadeDatePrimitive;

pub type Error = Box<dyn core::error::Error>;
pub type Result<T> = core::result::Result<T, Error>;

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
    pub hardware_metadata: Option<String>,
    // TODO: implement this!
    // pub serial_number: Option<String>,
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
            hardware_metadata: Some(value.hardware_metadata.clone()),
        }
    }
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
        jade_disk.hardware_metadata // TODO: implement this!
                                    // jade_disk.serial_number
    )
    .execute(pool)
    .await?
    .last_insert_id();

    Ok(jade_disk_id)
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
        jade_disk.hardware_metadata,
        jade_disk.jade_disk_id // TODO: implement this!
                               // jade_disk.serial_number
    )
    .execute(pool)
    .await?
    .rows_affected();

    Ok(rows_affected)
}

//---------------------------------------------------------------------------
//---------------------------------------------------------------------------
//---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_always_succeed() {
        assert!(true);
    }
}
