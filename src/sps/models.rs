// models.rs

use diesel::deserialize::FromSql;
use diesel::mysql::Mysql;
use diesel::prelude::*;

pub const MYSQL_FALSE: [u8; 1] = [0x00];
pub const MYSQL_TRUE: [u8; 1] = [0x01];

#[derive(Clone, Copy, Debug)]
pub struct BitBool(pub bool);

impl From<bool> for BitBool {
    fn from(value: bool) -> Self {
        BitBool(value)
    }
}

impl From<BitBool> for bool {
    fn from(value: BitBool) -> Self {
        value.0
    }
}

impl FromSql<diesel::sql_types::Binary, Mysql> for BitBool {
    fn from_sql(
        bytes: <Mysql as diesel::backend::Backend>::RawValue<'_>,
    ) -> diesel::deserialize::Result<Self> {
        let b = bytes.as_bytes();
        if !b.is_empty() {
            if b[0] & 0x01 == 0x01 {
                return Ok(BitBool(true));
            }
            return Ok(BitBool(false));
        }
        Ok(BitBool(false))
    }
}

// --------------------------------------------------------------------------
// -- jade_host -------------------------------------------------------------
// --------------------------------------------------------------------------

#[allow(dead_code)] // TODO: revisit this later
#[derive(Clone, Queryable, Selectable)]
#[diesel(table_name = crate::sps::schema::jade_host)]
#[diesel(check_for_backend(diesel::mysql::Mysql))]
pub struct JadeHost {
    jade_host_id: i64,
    allow_job_claim: Option<BitBool>,
    allow_job_work: Option<BitBool>,
    allow_open_job_claim: Option<BitBool>,
    date_created: Option<chrono::NaiveDateTime>,
    date_heartbeat: Option<chrono::NaiveDateTime>,
    date_updated: Option<chrono::NaiveDateTime>,
    host_name: Option<String>,
    satellite_capable: Option<BitBool>,
    version: Option<i64>,
}

#[derive(Insertable)]
#[diesel(table_name = crate::sps::schema::jade_host)]
#[diesel(check_for_backend(diesel::mysql::Mysql))]
pub struct NewJadeHost<'a> {
    pub allow_job_claim: [u8; 1],
    pub allow_job_work: [u8; 1],
    pub allow_open_job_claim: [u8; 1],
    pub date_created: chrono::NaiveDateTime,
    pub date_heartbeat: chrono::NaiveDateTime,
    pub date_updated: chrono::NaiveDateTime,
    pub host_name: &'a str,
    pub satellite_capable: [u8; 1],
    pub version: i64,
}
