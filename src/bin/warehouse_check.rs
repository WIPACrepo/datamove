// warehouse_check.rs

/// warehouse_check is a utility to perform mass verification of files
/// located in the data warehouse. While it can be run stand-alone, it
/// is intended to be run with multiple replicas in Kubernetes. Each
/// replica takes a portion of the workload; i.e. running sha512sum
/// commands on hundreds (or thousands) of large files.
///
/// Motivation
///
/// In late April 2023, mfrere identified some files in the data
/// warehouse with length 0 bytes. The root cause was eventually traced
/// to a bug in the deployment of the jadenorth data warehouser.
/// The Kubernetes pod running the data warehouser would be killed
/// but the data wouldn't be flushed to the data warehouse disk.
/// Files 'verified' as good by the data warehouser were stored
/// incorrectly on disk.
///
/// A fix in jadenorth to force a sync of the data to the disk before
/// calculating the verification checksum fixed this problem. All
/// of the zero length files were fixed. However, on Jan 2nd, 2024,
/// Rob Snihur identified a few PFFilt files that were corrupt.
///
/// These files seem to be corrupted around that same time period.
/// While they have a length (unlike the zero length files), the
/// contents of the file appear to be all zero bytes (0x00).
///
/// Solution
///
/// Both jade (SPS) and jadenorth store filename and checksum
/// information in their databases. By generating a checksum file, any
/// files can be verified as correctly stored in the warehouse or
/// corrupted.
///
/// After identification of corrupted files, good copies can be
/// restored from archival sources like a physical disk, or tape backup
/// at NERSC and/or TACC.
///
/// The task then is to distribute the workload of running sha512sum
/// on ~500K files in the data warehouse to ensure the integrity of
/// all of those files. warehouse_check does this by grabbing a file
/// that would be provided under the `--check` flag to the sha512sum
/// command, and running it.
///
/// The initial check file can be generated from a MySQL query against
/// the database. Using the `split` command, it can be divided into
/// as many work units as desired. By running multiple replicas of
/// warehouse_check in Kubernetes, each can consume work units
/// independently until all of the work units are exhausted.
///
/// Files that do not pass verification can send the work unit to a
/// quarantine directory along with a note of the file(s) that caused
/// the work unit to be quarantined for further examination.
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

use tracing::{error, info, trace, warn};

use wipac_datamove::adhoc::utils::next_file;
use wipac_datamove::error::{DatamoveError, Result};

#[derive(Debug)]
pub struct Context {
    pub debug_delay_seconds: u64,
    pub inbox_dir: String,
    pub outbox_dir: String,
    pub quarantine_dir: String,
    pub run_once_and_die: bool,
    pub work_dir: String,
    pub work_sleep_seconds: u64,
}

pub fn load_context() -> Context {
    // parse environment that needs parsing
    let debug_delay_seconds_str = env::var("DEBUG_DELAY_SECONDS").unwrap_or("0".to_string());
    let debug_delay_seconds: u64 = debug_delay_seconds_str
        .parse()
        .expect("Unable to parse DEBUG_DELAY_SECONDS as an integer (u64)");
    let run_once_and_die_str =
        env::var("RUN_ONCE_AND_DIE").expect("RUN_ONCE_AND_DIE environment variable not set");
    let run_once_and_die: bool = run_once_and_die_str
        .parse()
        .expect("Unable to parse RUN_ONCE_AND_DIE as a bool");
    let work_sleep_seconds_str =
        env::var("WORK_SLEEP_SECONDS").expect("WORK_SLEEP_SECONDS environment variable not set");
    let work_sleep_seconds: u64 = work_sleep_seconds_str
        .parse()
        .expect("Unable to parse WORK_SLEEP_SECONDS as an integer (u64)");
    // build the application context
    Context {
        debug_delay_seconds,
        inbox_dir: env::var("INBOX_DIR").expect("INBOX_DIR environment variable not set"),
        outbox_dir: env::var("OUTBOX_DIR").expect("OUTBOX_DIR environment variable not set"),
        quarantine_dir: env::var("QUARANTINE_DIR")
            .expect("QUARANTINE_DIR environment variable not set"),
        run_once_and_die,
        work_dir: env::var("WORK_DIR").expect("WORK_DIR environment variable not set"),
        work_sleep_seconds,
    }
}

fn build_note_path(quarantine_dir: &Path, file: &Path) -> PathBuf {
    // get the basename of the file "/path/to/my.data" -> "my.data"
    let basename = file.file_name().unwrap(); // Get the basename of the file
    let mut note_name = basename.to_os_string();
    // append the ".note" extension to the filename
    note_name.push(".note");
    // compute the full path of the file in the quarantine directory
    quarantine_dir.join(note_name)
}

pub fn main() -> Result<()> {
    // enable logging
    env_logger::init();
    // load the application context
    let context = load_context();
    trace!("context: {context:#?}");
    // get our work paths all set up
    let inbox_dir = Path::new(&context.inbox_dir);
    let outbox_dir = Path::new(&context.outbox_dir);
    let quarantine_dir = Path::new(&context.quarantine_dir);
    let work_dir = Path::new(&context.work_dir);
    // set up some things we'd like to keep track of
    let mut sent_to_quarantine = false;
    let mut work_count = 0;
    // いつまでも
    loop {
        // try to claim the next file to work on it
        // errors here terminate the program
        match next_file(inbox_dir, work_dir)? {
            // if we managed to grab a file
            Some(file) => {
                // increment the work counter
                work_count += 1;
                // attempt to process the file
                match process_file(&context, &file) {
                    // processing successful
                    Ok(_) => {
                        // log about finishing the file
                        info!("Finished processing: {file:?}");
                        // log about moving the finished file to the outbox
                        let dest = outbox_dir.join(file.file_name().unwrap());
                        info!("Moving {file:?} -> {dest:?}");
                        // move the file to the outbox
                        // errors here terminate the program
                        fs::rename(&file, &dest)?;
                    }
                    // processing NOT successful
                    Err(e) => {
                        // log about finishing the file
                        error!("Error processing {file:?}: {e}");
                        // log about moving the finished file to the quarantine directory
                        let dest = quarantine_dir.join(file.file_name().unwrap());
                        info!("Moving {file:?} -> {dest:?}");
                        // move the file to the quarantine directory
                        // errors here terminate the program
                        fs::rename(&file, &dest)?;
                        // set the flag indicating that we sent something to quarantine
                        sent_to_quarantine = true;
                    }
                }
            }
            // whoops, no work left to do
            None => {
                // log about how much work we did
                info!("Processed {work_count} file(s) this cycle.");
                // reset the work counter
                work_count = 0;
                // if we were configured to run a single cycle
                if context.run_once_and_die {
                    warn!("RUN_ONCE_AND_DIE = true; Work cycle is now complete.");
                    // if we had to send anything to quarantine
                    if sent_to_quarantine {
                        // exit with code = 1 (error)
                        std::process::exit(1);
                    }
                    // all good, exit with code = 0 (success)
                    std::process::exit(0);
                }
                // otherwise, just sleep until we check for more work
                let work_sleep_seconds = context.work_sleep_seconds;
                thread::sleep(Duration::from_secs(work_sleep_seconds));
            }
        }
    }
}

fn process_file(context: &Context, file: &Path) -> Result<()> {
    // log about processing the file
    info!("Processing: {file:?}");
    // run `sha512sum --check {file}` and capture the output and exit code
    let output = Command::new("sha512sum")
        .arg("--check")
        .arg(file)
        .output()?;
    // if the environment requested a delay for debugging purposes
    if context.debug_delay_seconds > 0 {
        // sleep the required number of seconds to allow for debugging
        thread::sleep(Duration::from_secs(context.debug_delay_seconds));
    }
    // if the command was not a smashing success
    if !output.status.success() {
        // determine the path to our note file: "{context.quarantine_dir}/{file}.note"
        let quarantine_dir = Path::new(&context.quarantine_dir);
        let note_path = build_note_path(quarantine_dir, file);
        // write the command output to our note file
        let mut note_file = fs::File::create(&note_path)?;
        note_file.write_all(&output.stdout)?;
        note_file.write_all("\n\n\n".as_bytes())?;
        note_file.write_all(&output.stderr)?;
        note_file.flush()?;
        // indicate to the caller that the command didn't succeed
        let exit_code = output.status.code().unwrap_or(-1);
        let msg = format!("Exit code {exit_code}: See {note_path:?}");
        return Err(DatamoveError::Critical(msg));
    }
    // indicate to the caller that we processed the file successfully
    Ok(())
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
