// disk_label.rs

use chrono::NaiveDateTime;
use sqlx::types::time::PrimitiveDateTime;
use sqlx::{FromRow, MySql, MySqlPool, Transaction};

use crate::sps::jade_db::service::disk_label::JadeDiskLabel;
use crate::sps::jade_db::utils::JadeDatePrimitive;

pub type Error = Box<dyn core::error::Error>;
pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, FromRow)]
pub struct MySqlJadeDiskLabel {
    pub jade_disk_label_id: i64,
    pub version: Option<i64>,
    pub date_created: Option<PrimitiveDateTime>,
    pub date_updated: Option<PrimitiveDateTime>,
    pub disk_archive_uuid: Option<String>,
    pub copy_id: Option<i32>,
    pub disk_archive_year: Option<i32>,
    pub disk_archive_sequence: Option<i32>,
}

impl From<&JadeDiskLabel> for MySqlJadeDiskLabel {
    fn from(value: &JadeDiskLabel) -> Self {
        fn convert_date(ndt: NaiveDateTime) -> PrimitiveDateTime {
            let jade_date: JadeDatePrimitive = ndt.into();
            jade_date.into()
        }

        MySqlJadeDiskLabel {
            jade_disk_label_id: value.jade_disk_label_id,
            version: Some(value.version),
            date_created: Some(convert_date(value.date_created)),
            date_updated: Some(convert_date(value.date_updated)),
            disk_archive_uuid: Some(value.disk_archive_uuid.clone()),
            copy_id: Some(value.copy_id),
            disk_archive_year: Some(value.disk_archive_year),
            disk_archive_sequence: Some(value.disk_archive_sequence),
        }
    }
}

//---------------------------------------------------------------------------------------------------------------------
//---------------------------------------------------------------------------------------------------------------------
//---------------------------------------------------------------------------------------------------------------------

pub async fn create(pool: &MySqlPool, jade_disk_label: &MySqlJadeDiskLabel) -> Result<u64> {
    let jade_disk_label_id = sqlx::query!(
        r#"
            INSERT INTO jade_disk_label (
                version,
                date_created,
                date_updated,
                disk_archive_uuid,
                copy_id,
                disk_archive_year,
                disk_archive_sequence
            ) VALUES (
                ?,
                ?,
                ?,
                ?,
                ?,
                ?,
                ?
            )
        "#,
        jade_disk_label.version,
        jade_disk_label.date_created,
        jade_disk_label.date_updated,
        jade_disk_label.disk_archive_uuid,
        jade_disk_label.copy_id,
        jade_disk_label.disk_archive_year,
        jade_disk_label.disk_archive_sequence,
    )
    .execute(pool)
    .await?
    .last_insert_id();

    Ok(jade_disk_label_id)
}

pub async fn find_by_id(
    pool: &MySqlPool,
    jade_disk_label_id: i64,
) -> Result<Option<MySqlJadeDiskLabel>> {
    let result = sqlx::query_as!(
        MySqlJadeDiskLabel,
        r#"SELECT * FROM jade_disk_label WHERE jade_disk_label_id = ?"#,
        jade_disk_label_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(result)
}

pub async fn get_next_label(
    pool: &MySqlPool,
    disk_archive_uuid: &str,
    copy_id: u64,
    disk_archive_year: u64,
) -> Result<u64> {
    // create a transaction to update the sequence number atomically
    let mut tx: Transaction<MySql> = pool.begin().await?;
    // try to get the current sequence number with a row lock
    let current_sequence = sqlx::query_scalar!(
        r#"
        SELECT disk_archive_sequence
        FROM jade_disk_label
        WHERE disk_archive_uuid = ?
          AND copy_id = ?
          AND disk_archive_year = ?
        FOR UPDATE
        "#,
        disk_archive_uuid,
        copy_id,
        disk_archive_year,
    )
    .fetch_optional(&mut *tx)
    .await?;
    // depending if we got a row back or not...
    let previous_sequence = match current_sequence {
        // if there is already a sequence
        Some(seq) => {
            let curr_sequence = seq.expect("jade_disk_label.disk_archive_sequence IS null") as u64;
            let next_sequence = curr_sequence + 1;
            // update existing row with new sequence
            sqlx::query!(
                r#"
                UPDATE jade_disk_label
                SET
                    disk_archive_sequence = ?,
                    date_updated = NOW()
                WHERE disk_archive_uuid = ?
                  AND copy_id = ?
                  AND disk_archive_year = ?
                "#,
                next_sequence,
                disk_archive_uuid,
                copy_id,
                disk_archive_year
            )
            .execute(&mut *tx)
            .await?;
            // our return value will be the sequence as we found it in the database
            curr_sequence
        }
        None => {
            // no existing row, create one with initial sequence = 1
            sqlx::query!(
                r#"
                INSERT INTO jade_disk_label (
                    version,
                    date_created,
                    date_updated,
                    disk_archive_uuid,
                    copy_id,
                    disk_archive_year,
                    disk_archive_sequence
                ) VALUES (
                    1,
                    NOW(),
                    NOW(),
                    ?,
                    ?,
                    ?,
                    1
                )
                "#,
                disk_archive_uuid,
                copy_id,
                disk_archive_year
            )
            .execute(&mut *tx)
            .await?;
            // our return value will be 0; the first number in a new sequence
            0
        }
    };
    // commit the transaction to update the sequence number in the database
    tx.commit().await?;
    // return the sequence number we originally found to the caller
    Ok(previous_sequence)
}

pub async fn save(pool: &MySqlPool, jade_disk_label: &MySqlJadeDiskLabel) -> Result<u64> {
    let rows_affected = sqlx::query!(
        r#"
            UPDATE jade_disk_label
            SET version = ?,
                date_created = ?,
                date_updated = ?,
                disk_archive_uuid = ?,
                copy_id = ?,
                disk_archive_year = ?,
                disk_archive_sequence = ?
            WHERE jade_disk_label_id = ?
        "#,
        jade_disk_label.version,
        jade_disk_label.date_created,
        jade_disk_label.date_updated,
        jade_disk_label.disk_archive_uuid,
        jade_disk_label.copy_id,
        jade_disk_label.disk_archive_year,
        jade_disk_label.disk_archive_sequence,
        jade_disk_label.jade_disk_label_id
    )
    .execute(pool)
    .await?
    .rows_affected();

    Ok(rows_affected)
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
