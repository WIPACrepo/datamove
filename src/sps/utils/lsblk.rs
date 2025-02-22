// lsblk.rs

use serde::Deserialize;
use std::process::Command;

// lsblk --json -o name,mountpoint,serial
// {
//     "blockdevices": [
//        {"name": "sda", "mountpoint": null, "serial": "005517ac0ba160422400d96b51f098cd",
//           "children": [
//              {"name": "sda1", "mountpoint": null, "serial": null},
//              {"name": "sda2", "mountpoint": "/boot", "serial": null},
//              {"name": "sda3", "mountpoint": "[SWAP]", "serial": null},
//              {"name": "sda4", "mountpoint": null, "serial": null},
//              {"name": "sda5", "mountpoint": "/", "serial": null}
//           ]
//        },
//        {"name": "sdb", "mountpoint": "/mnt/data", "serial": "003e93d5343965422400d96b51f098cd"},
//        {"name": "sdc", "mountpoint": null, "serial": "PL1321LAGAPN4H",
//           "children": [
//              {"name": "sdc1", "mountpoint": "/mnt/slot4", "serial": null}
//           ]
//        },
//        {"name": "sdd", "mountpoint": "/mnt/slot3", "serial": "PL2331LAGSU86J"},
//        {"name": "sde", "mountpoint": "/mnt/slot1", "serial": "WD-WCAVY7251869"},
//        {"name": "sdf", "mountpoint": "/mnt/slot7", "serial": "PL2331LAGRX1JJ"},
//        {"name": "sdg", "mountpoint": "/mnt/slot8", "serial": "ZL2LS8920000C2050W7P"},
//        {"name": "sdh", "mountpoint": "/mnt/slot6", "serial": "ZL22PHAL0000C0048NEQ"},
//        {"name": "sdi", "mountpoint": "/mnt/slot2", "serial": "W1E40X6R"},
//        {"name": "sdj", "mountpoint": "/mnt/slot5", "serial": "S1X09XKP"}
//     ]
// }

#[derive(Deserialize, Debug)]
struct BlockDevice {
    mountpoint: Option<String>,
    serial: Option<String>,
    children: Option<Vec<BlockDevice>>,
}

#[derive(Deserialize, Debug)]
struct LsblkOutput {
    blockdevices: Vec<BlockDevice>,
}

/// Runs `lsblk --json -o mountpoint,serial` and finds the serial for a given
/// mountpoint.
pub fn get_serial_for_mountpoint(mountpoint: &str) -> Option<String> {
    // run `lsblk --json -o mountpoint,serial`
    let output = Command::new("lsblk")
        .args(["--json", "-o", "mountpoint,serial"])
        .output()
        .ok()?;
    // parse JSON output
    let lsblk_json: LsblkOutput = serde_json::from_slice(&output.stdout).ok()?;
    // recursively search for the mountpoint and return its serial
    fn find_serial(devices: &[BlockDevice], mountpoint: &str) -> Option<String> {
        // for each device in the list
        for device in devices {
            // if it's got a mountpoint
            if let Some(mp) = &device.mountpoint {
                // and the mountpoint is the one we're looking for
                if mp == mountpoint {
                    // return the serial of that mountpoint
                    return device.serial.clone();
                }
            }
            // if it's got children
            if let Some(children) = &device.children {
                // recursively search the children
                if let Some(serial) = find_serial(children, mountpoint) {
                    // if we found a serial number in the children, return it
                    return Some(serial);
                }
            }
        }
        // we didn't find a serial for that mountpoint
        None
    }
    // find the serial for the provided mountpoint in the output
    find_serial(&lsblk_json.blockdevices, mountpoint)
}
