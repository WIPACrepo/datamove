// TODO: delete me
// database.rs

// use chrono::Utc;
// use diesel::{insert_into, update};
// use diesel::prelude::*;
// use log::{error, info, trace, warn};

// use crate::sps::context::Context;
// use crate::sps::models::{JadeDisk, JadeHost, NewJadeHost};
// use crate::sps::models::{MYSQL_FALSE, MYSQL_TRUE};

// pub type Error = Box<dyn core::error::Error>;
// pub type Result<T> = core::result::Result<T, Error>;

// // --------------------------------------------------------------------------
// // -- Database API ----------------------------------------------------------
// // --------------------------------------------------------------------------

// pub fn ensure_host(context: &Context) -> Result<JadeHost> {
//     trace!("ensure_host({})", context.hostname);
//     let mut conn = match context.db.lock() {
//         Ok(conn) => conn,
//         Err(x) => return Err(format!("Unable to lock MysqlConnection: {x}").into()),
//     };
//     let hostname = &context.hostname;

//     // if we already have this host in the database
//     if let Some(host) = select_host_by_hostname(&mut conn, hostname)? {
//         // return it to the caller
//         return Ok(host);
//     }

//     // otherwise, we need to create the host in the database
//     insert_host(&mut conn, hostname)?;

//     // now let's return the host that we just created
//     if let Some(host) = select_host_by_hostname(&mut conn, hostname)? {
//         return Ok(host);
//     }

//     // oops, something has gone very wrong...
//     error!("Unable to find row for host_name '{hostname}' after insert into table jade_host");
//     Err("Unable to ensure_host".into())
// }

// // --------------------------------------------------------------------------
// // -- jade_disk -------------------------------------------------------------
// // --------------------------------------------------------------------------

// pub fn close_disk_by_uuid(
//     conn: &mut MysqlConnection,
//     disk_uuid: &str,
// ) -> Result<usize> {
//     trace!("close_disk_by_uuid({disk_uuid})");
//     use crate::sps::schema::jade_disk::dsl::*;

//     let now = Utc::now().naive_utc();

//     // query the DB to update the disk by closing it
//     // SQL: update jade_disk set closed=true, date_updated=now() where uuid = $disk_uuid;
//     let count = update(jade_disk)
//         .set((closed.eq(MYSQL_TRUE), date_updated.eq(now)))
//         .filter(uuid.eq(disk_uuid))
//         .execute(conn)?;

//     // if we were able to update the single row, log our success
//     if count == 1 {
//         info!("Updated row in table jade_disk for uuid '{disk_uuid}'");
//         return Ok(count);
//     }

//     // oops, something went wrong...
//     error!("Unable to update row in table jade_disk for uuid '{disk_uuid}' ({count})");
//     Err("DB: update jade_disk".into())
// }

// pub fn select_disk_by_uuid(
//     conn: &mut MysqlConnection,
//     find_uuid: &str,
// ) -> Result<Option<JadeDisk>> {
//     trace!("select_disk_by_uuid({find_uuid})");
//     use crate::sps::schema::jade_disk::dsl::*;

//     // query the DB to find the row for the provided hostname
//     // SQL: select * from jade_disk where uuid = $uuid order jade_host_id limit 1;
//     let disks = jade_disk
//         .select(JadeDisk::as_select())
//         .filter(uuid.eq(find_uuid))
//         .order(jade_disk_id)
//         .limit(1)
//         .load(conn)?;

//     // if we got more than one row back, log a warning about that
//     if disks.len() > 1 {
//         warn!("Multiple rows returned from jade_disk for uuid = '{find_uuid}'")
//     }

//     // we found it (hopefully only one), so return it to the caller
//     if !disks.is_empty() {
//         return Ok(Some(disks[0].clone()));
//     }

//     // oops, we did not find anything
//     Ok(None)
// }

// // --------------------------------------------------------------------------
// // -- jade_host -------------------------------------------------------------
// // --------------------------------------------------------------------------

// fn insert_host(conn: &mut MysqlConnection, hostname: &str) -> Result<usize> {
//     trace!("insert_host({hostname})");
//     use crate::sps::schema::jade_host::dsl::*;

//     let now = Utc::now().naive_utc();

//     // create the host to be added to the database
//     let new_host = NewJadeHost {
//         allow_job_claim: MYSQL_FALSE,
//         allow_job_work: MYSQL_FALSE,
//         allow_open_job_claim: MYSQL_FALSE,
//         date_created: now,
//         date_heartbeat: now,
//         date_updated: now,
//         host_name: hostname,
//         satellite_capable: MYSQL_TRUE,
//         version: 0,
//     };

//     // insert the row into the database
//     let count = insert_into(jade_host).values(&new_host).execute(conn)?;

//     // if we were able to insert the single row, log our success
//     if count == 1 {
//         info!("Created row in table jade_host for host_name '{hostname}'");
//         return Ok(count);
//     }

//     // oops, something went wrong...
//     error!("Unable to insert row into table jade_host for host_name '{hostname}' ({count})");
//     Err("DB: insert into jade_host".into())
// }

// fn select_host_by_hostname(conn: &mut MysqlConnection, hostname: &str) -> Result<Option<JadeHost>> {
//     trace!("select_host_by_hostname({hostname})");
//     use crate::sps::schema::jade_host::dsl::*;

//     // query the DB to find the row for the provided hostname
//     // SQL: select * from jade_host where host_name = $hostname order jade_host_id limit 1;
//     let hosts = jade_host
//         .select(JadeHost::as_select())
//         .filter(host_name.eq(hostname))
//         .order(jade_host_id)
//         .limit(1)
//         .load(conn)?;

//     // if we got more than one row back, log a warning about that
//     if hosts.len() > 1 {
//         warn!("Multiple rows returned from jade_host for host_name = '{hostname}'")
//     }

//     // we found it (hopefully only one), so return it to the caller
//     if !hosts.is_empty() {
//         return Ok(Some(hosts[0].clone()));
//     }

//     // oops, we did not find anything
//     Ok(None)
// }
