// host.rs

use chrono::{NaiveDateTime, Utc};
use sqlx::MySqlPool;
use tracing::{error, trace};

use crate::sps::jade_db::repo::host::create as repo_create;
use crate::sps::jade_db::repo::host::find_by_host_name as repo_find_by_host_name;
use crate::sps::jade_db::repo::host::MySqlJadeHost;
use crate::sps::jade_db::utils::JadeDateNaive;

use crate::error::{DatamoveError, Result};

#[derive(Clone)]
pub struct JadeHost {
    pub jade_host_id: i64,
    pub allow_job_claim: bool,
    pub allow_job_work: bool,
    pub allow_open_job_claim: bool,
    pub date_created: NaiveDateTime,
    pub date_heartbeat: NaiveDateTime,
    pub date_updated: NaiveDateTime,
    pub host_name: String,
    pub satellite_capable: bool,
    pub version: i64,
}

impl From<MySqlJadeHost> for JadeHost {
    fn from(value: MySqlJadeHost) -> Self {
        fn convert_date(
            pdt: Option<sqlx::types::time::PrimitiveDateTime>,
            field_name: &str,
        ) -> NaiveDateTime {
            let jade_date: JadeDateNaive = pdt
                .unwrap_or_else(|| panic!("jade_host.{} IS null", field_name))
                .into();
            jade_date.into()
        }

        JadeHost {
            jade_host_id: value.jade_host_id,
            allow_job_claim: value
                .allow_job_claim
                .expect("jade_host.allow_job_claim IS null")
                & 0x1
                == 0x1,
            allow_job_work: value
                .allow_job_work
                .expect("jade_host.allow_job_work IS null")
                & 0x1
                == 0x1,
            allow_open_job_claim: value
                .allow_open_job_claim
                .expect("jade_host.allow_open_job_claim IS null")
                & 0x1
                == 0x1,
            date_created: convert_date(value.date_created, "date_created"),
            date_heartbeat: convert_date(value.date_heartbeat, "date_heartbeat"),
            date_updated: convert_date(value.date_updated, "date_updated"),
            host_name: value.host_name.expect("jade_host.host_name IS null"),
            satellite_capable: value
                .satellite_capable
                .expect("jade_host.satellite_capable IS null")
                & 0x1
                == 0x1,
            version: value.version.expect("jade_host.version IS null"),
        }
    }
}

pub async fn ensure_host(pool: &MySqlPool, hostname: &str) -> Result<JadeHost> {
    trace!("ensure_host({hostname})");

    // ask the database if we already have this host in the database
    match find_by_host_name(pool, hostname).await {
        // if we already have this host in the database
        Ok(jade_host) => {
            // return it to the caller
            return Ok(jade_host);
        }
        // if we don't have this host in the database
        Err(e) => {
            // log about it and go into the creation bit
            error!("Unable to find hostname '{hostname}' due to: {e}");
            error!("Will attempt to create host for hostname '{hostname}'");
        }
    }

    // otherwise, we need to create the host in the database
    let now = Utc::now().naive_utc();
    let jade_host = JadeHost {
        jade_host_id: 0,
        allow_job_claim: false,
        allow_job_work: false,
        allow_open_job_claim: false,
        date_created: now,
        date_heartbeat: now,
        date_updated: now,
        host_name: hostname.to_string(),
        satellite_capable: true,
        version: 0,
    };
    let host: MySqlJadeHost = jade_host.into();
    repo_create(pool, &host).await?;

    // now let's return the host that we just created
    find_by_host_name(pool, hostname).await
}

pub async fn find_by_host_name(pool: &MySqlPool, find_host_name: &str) -> Result<JadeHost> {
    // try to locate the host by host_name in the database
    match repo_find_by_host_name(pool, find_host_name).await {
        // if we got a result back from the database
        Ok(host) => {
            if let Some(host) = host {
                // convert it to a service layer JadeHost and return it to the caller
                let jade_host: JadeHost = host.into();
                Ok(jade_host)
            } else {
                // otherwise log the missing host as an error and return an Err Result
                let msg = format!(
                    "Database table jade_host has no entry for host_name '{find_host_name}'."
                );
                error!(msg);
                Err(DatamoveError::Critical(msg))
            }
        }
        // whoops, something went wrong in the database layer, better log about that
        Err(e) => {
            let msg = format!(
                "Unable to read database table jade_host for host_name '{find_host_name}': {e}."
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
