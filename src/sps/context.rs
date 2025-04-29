// context.rs

use std::fs;

use sqlx::{mysql::MySqlPoolOptions, MySqlPool};
use tracing::{error, info};

use crate::config::DatamoveConfiguration;

#[derive(Clone)]
pub struct Context {
    pub config: DatamoveConfiguration,
    pub db_pool: MySqlPool,
    pub hostname: String,
}

pub fn load_context() -> Context {
    // load the path to the configuration file
    let datamove_config_path = match std::env::var("DATAMOVE_CONFIG") {
        Ok(config_path) => config_path,
        Err(e) => {
            error!("Expected environment variable: DATAMOVE_CONFIG: {e}");
            panic!("Expected environment variable: DATAMOVE_CONFIG");
        }
    };
    // load the TOML configuration file
    let datamove_config_text = match fs::read_to_string(&datamove_config_path) {
        Ok(config_text) => config_text,
        Err(e) => {
            error!("Failed to read configuration file: '{datamove_config_path}': {e}");
            panic!("Failed to read configuration file: '{datamove_config_path}'");
        }
    };
    // deserialize the TOML configuration file
    let datamove_config: DatamoveConfiguration = match toml::from_str(&datamove_config_text) {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to parse configuration text: {e}");
            panic!("Failed to parse configuration text");
        }
    };

    // load database configuration from the configuration object
    let username = &datamove_config.jade_database.username;
    let password = &datamove_config.jade_database.password;
    let host = &datamove_config.jade_database.host;
    let port = datamove_config.jade_database.port;
    let database_name = &datamove_config.jade_database.database_name;
    let database_url = format!("mysql://{username}:{password}@{host}:{port}/{database_name}");

    // db: set up the database connection
    // let db_pool = MySqlPool::connect_lazy(&database_url)
    //     .expect("Unable to create MySqlPool to connect to the database");
    let db_pool = match MySqlPoolOptions::new()
        .min_connections(15)
        .max_connections(20)
        .test_before_acquire(false)
        .connect_lazy(&database_url)
    {
        Ok(pool) => pool,
        Err(e) => {
            error!("Unable to create MySqlPool to connect to the database: {e}");
            panic!("Unable to create MySqlPool to connect to the database");
        }
    };

    // hostname: determine the hostname running the process
    let full_hostname = hostname::get().expect("Failed to get hostname");
    let lossy_hostname = full_hostname.to_string_lossy();
    let hostname = match lossy_hostname.split('.').next() {
        Some(short_hostname) => short_hostname,
        None => {
            error!("Unable to parse short hostname");
            panic!("Unable to parse short hostname");
        }
    };
    info!("hostname: {}", hostname);

    // return the application Context object to the caller
    Context {
        config: datamove_config.clone(),
        db_pool,
        hostname: hostname.to_string(),
    }
}
