// workbench.rs

use log::trace;
use wipac_datamove::ensure_minimum_usize;

fn main() {
    env_logger::init();
    ensure_minimum_usize();
    trace!("Hello, datamove!");
}
