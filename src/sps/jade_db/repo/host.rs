// host.rs

use chrono::NaiveDateTime;
use sqlx::types::time::PrimitiveDateTime;
use sqlx::{FromRow, MySqlPool};

use crate::sps::jade_db::service::host::JadeHost;
use crate::sps::jade_db::utils::JadeDatePrimitive;

pub type Error = Box<dyn core::error::Error>;
pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, FromRow)]
pub struct MySqlJadeHost {
    pub jade_host_id: i64,
    pub allow_job_claim: Option<u8>,
    pub allow_job_work: Option<u8>,
    pub allow_open_job_claim: Option<u8>,
    pub date_created: Option<PrimitiveDateTime>,
    pub date_heartbeat: Option<PrimitiveDateTime>,
    pub date_updated: Option<PrimitiveDateTime>,
    pub host_name: Option<String>,
    pub satellite_capable: Option<u8>,
    pub version: Option<i64>,
}

impl From<JadeHost> for MySqlJadeHost {
    fn from(value: JadeHost) -> Self {
        fn convert_date(ndt: NaiveDateTime) -> PrimitiveDateTime {
            let jade_date: JadeDatePrimitive = ndt.into();
            jade_date.into()
        }

        MySqlJadeHost {
            jade_host_id: value.jade_host_id,
            allow_job_claim: if value.allow_job_claim {
                Some(0x01)
            } else {
                Some(0x00)
            },
            allow_job_work: if value.allow_job_work {
                Some(0x01)
            } else {
                Some(0x00)
            },
            allow_open_job_claim: if value.allow_open_job_claim {
                Some(0x01)
            } else {
                Some(0x00)
            },
            date_created: Some(convert_date(value.date_created)),
            date_heartbeat: Some(convert_date(value.date_heartbeat)),
            date_updated: Some(convert_date(value.date_updated)),
            host_name: Some(value.host_name.clone()),
            satellite_capable: if value.satellite_capable {
                Some(0x01)
            } else {
                Some(0x00)
            },
            version: Some(value.version),
        }
    }
}

pub async fn create(pool: &MySqlPool, jade_host: &MySqlJadeHost) -> Result<u64> {
    let jade_host_id = sqlx::query!(
        r#"
            INSERT INTO jade_host (
                allow_job_claim,
                allow_job_work,
                allow_open_job_claim,
                date_created,
                date_heartbeat,
                date_updated,
                host_name,
                satellite_capable,
                version
            ) VALUES (
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
        jade_host.allow_job_claim,
        jade_host.allow_job_work,
        jade_host.allow_open_job_claim,
        jade_host.date_created,
        jade_host.date_heartbeat,
        jade_host.date_updated,
        jade_host.host_name,
        jade_host.satellite_capable,
        jade_host.version,
    )
    .execute(pool)
    .await?
    .last_insert_id();

    Ok(jade_host_id)
}

pub async fn find_by_id(pool: &MySqlPool, jade_host_id: i64) -> Result<Option<MySqlJadeHost>> {
    let result = sqlx::query_as!(
        MySqlJadeHost,
        r#"SELECT * FROM jade_host WHERE jade_host_id = ?"#,
        jade_host_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(result)
}

pub async fn find_by_host_name(pool: &MySqlPool, host_name: &str) -> Result<Option<MySqlJadeHost>> {
    let result = sqlx::query_as!(
        MySqlJadeHost,
        r#"SELECT * FROM jade_host WHERE host_name = ?"#,
        host_name
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
    use super::*;

    #[test]
    fn test_always_succeed() {
        assert!(true);
    }
}
