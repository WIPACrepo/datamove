// hardware_metadata.rs

use std::process::Command;

use serde::Serialize;
use tracing::{error, info};

use crate::error::{DatamoveError, Result};

#[derive(Serialize)]
pub struct HardwareMetadata {
    pub metadata: Vec<String>,
}

pub fn get_hardware_metadata(archival_disk_path: &str) -> Result<String> {
    // Find the block device name for the given mount path
    let name = get_hardware_metadata_name(archival_disk_path)?;

    // Collect metadata from id, path, and uuid
    let mut metadata = Vec::new();
    metadata.extend(get_hardware_metadata_by_id(&name)?);
    metadata.extend(get_hardware_metadata_by_path(&name)?);
    metadata.extend(get_hardware_metadata_by_uuid(&name)?);

    // Serialize to JSON
    let hardware_metadata = HardwareMetadata { metadata };
    Ok(serde_json::to_string(&hardware_metadata)?)
}

fn get_hardware_metadata_by_id(name: &str) -> Result<Vec<String>> {
    run_ls_collect(
        name,
        "ls",
        &["-lrt", "--time-style=+%s", "/dev/disk/by-id"],
        "id",
    )
}

fn get_hardware_metadata_by_path(name: &str) -> Result<Vec<String>> {
    run_ls_collect(
        name,
        "ls",
        &["-lrt", "--time-style=+%s", "/dev/disk/by-path"],
        "path",
    )
}

fn get_hardware_metadata_by_uuid(name: &str) -> Result<Vec<String>> {
    run_ls_collect(
        name,
        "ls",
        &["-lrt", "--time-style=+%s", "/dev/disk/by-uuid"],
        "uuid",
    )
}

fn run_ls_collect(name: &str, cmd: &str, args: &[&str], typ: &str) -> Result<Vec<String>> {
    if name.is_empty() {
        return Ok(Vec::new());
    }

    let output = Command::new(cmd).args(args).output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    let mut results = Vec::new();
    for line in stdout.lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 3 {
            continue;
        }
        if cols[cols.len() - 1].ends_with(name) {
            info!(
                "Matched hardware name {} with {} {}",
                name,
                typ,
                cols[cols.len() - 3]
            );
            results.push(cols[cols.len() - 3].to_string());
        }
    }
    Ok(results)
}

fn get_hardware_metadata_name(archival_disk_path: &str) -> Result<String> {
    if archival_disk_path.is_empty() {
        return Err(DatamoveError::Other("archival_disk_path is empty".into()));
    }

    let output = Command::new("lsblk")
        .args(["--output", "name,mountpoint", "--raw"])
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 2 {
            continue;
        }
        if cols[1] == archival_disk_path {
            info!(
                "Found hardware metadata name for {}: {}",
                archival_disk_path, cols[0]
            );
            return Ok(cols[0].to_string());
        }
    }

    error!(
        "Unable to find hardware metadata name for {}",
        archival_disk_path
    );
    Err(DatamoveError::Other(
        format!(
            "No block device found for mount path {}",
            archival_disk_path
        )
        .into(),
    ))
}
