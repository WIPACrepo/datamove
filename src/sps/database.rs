// database.rs

use chrono::Utc;
use diesel::insert_into;
use diesel::prelude::*;
use log::{error, info, trace, warn};

use crate::sps::context::Context;
use crate::sps::models::{JadeHost, NewJadeHost};
use crate::sps::models::{MYSQL_FALSE, MYSQL_TRUE};

pub type Error = Box<dyn core::error::Error>;
pub type Result<T> = core::result::Result<T, Error>;

// --------------------------------------------------------------------------
// -- Database API ----------------------------------------------------------
// --------------------------------------------------------------------------

pub fn ensure_host(context: &mut Context) -> Result<JadeHost> {
    trace!("ensure_host({})", context.hostname);
    let conn = &mut context.db;
    let hostname = &context.hostname;

    // if we already have this host in the database
    if let Some(host) = select_host_by_hostname(conn, hostname)? {
        // return it to the caller
        return Ok(host);
    }

    // otherwise, we need to create the host in the database
    insert_host(conn, hostname)?;

    // now let's return the host that we just created
    if let Some(host) = select_host_by_hostname(conn, hostname)? {
        return Ok(host);
    }

    // oops, something has gone very wrong...
    error!("Unable to find row for host_name '{hostname}' after insert into table jade_host");
    Err("Unable to ensure_host".into())
}

// --------------------------------------------------------------------------
// -- jade_host -------------------------------------------------------------
// --------------------------------------------------------------------------

fn insert_host(conn: &mut MysqlConnection, hostname: &str) -> Result<usize> {
    trace!("insert_host({hostname})");
    use crate::sps::schema::jade_host::dsl::*;

    let now = Utc::now().naive_utc();

    // create the host to be added to the database
    let new_host = NewJadeHost {
        allow_job_claim: MYSQL_FALSE,
        allow_job_work: MYSQL_FALSE,
        allow_open_job_claim: MYSQL_FALSE,
        date_created: now,
        date_heartbeat: now,
        date_updated: now,
        host_name: hostname,
        satellite_capable: MYSQL_TRUE,
        version: 0,
    };

    // insert the row into the database
    let count = insert_into(jade_host).values(&new_host).execute(conn)?;

    // if we were able to insert the single row, log our success
    if count == 1 {
        info!("Created row in table jade_host for host_name '{hostname}'");
        return Ok(count);
    }

    // oops, something went wrong...
    error!("Unable to insert row into table jade_host for host_name '{hostname}' ({count})");
    Err("DB: insert into jade_host".into())
}

fn select_host_by_hostname(conn: &mut MysqlConnection, hostname: &str) -> Result<Option<JadeHost>> {
    trace!("select_host_by_hostname({hostname})");
    use crate::sps::schema::jade_host::dsl::*;

    // query the DB to find the row for the provided hostname
    // SQL: select * from jade_host where host_name = $hostname order jade_host_id limit 1;
    let hosts = jade_host
        .select(JadeHost::as_select())
        .filter(host_name.eq(hostname))
        .order(jade_host_id)
        .limit(1)
        .load(conn)?;

    // if we got more than one row back, log a warning about that
    if hosts.len() > 1 {
        warn!("Multiple rows returned from jade_host for host_name = '{hostname}'")
    }

    // we found it (hopefully only one), so return it to the caller
    if !hosts.is_empty() {
        return Ok(Some(hosts[0].clone()));
    }

    // oops, we did not find anything
    Ok(None)
}
