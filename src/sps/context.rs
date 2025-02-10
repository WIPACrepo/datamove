// context.rs

use log::info;
use sqlx::MySqlPool;
use std::fs;

use crate::config::DatamoveConfiguration;

#[derive(Clone)]
pub struct Context {
    pub config: DatamoveConfiguration,
    pub db_pool: MySqlPool,
    pub hostname: String,
}

pub fn load_context() -> Context {
    // load the path to the configuration file
    let datamove_config_path =
        std::env::var("DATAMOVE_CONFIG").expect("Expected environment variable: DATAMOVE_CONFIG");
    // load the TOML configuration file
    let datamove_config_text = fs::read_to_string(&datamove_config_path)
        .unwrap_or_else(|_| panic!("Failed to read configuration file: '{datamove_config_path}'"));
    // deserialize the TOML configuration file
    let datamove_config: DatamoveConfiguration =
        toml::from_str(&datamove_config_text).expect("Failed to parse configuration text");

    // load database configuration from the configuration object
    let username = &datamove_config.jade_database.username;
    let password = &datamove_config.jade_database.password;
    let host = &datamove_config.jade_database.host;
    let port = datamove_config.jade_database.port;
    let database_name = &datamove_config.jade_database.database_name;
    let database_url = format!("mysql://{username}:{password}@{host}:{port}/{database_name}");

    // db: set up the database connection
    let db_pool = MySqlPool::connect_lazy(&database_url)
        .expect("Unable to create MySqlPool to connect to the database");

    // hostname: determine the hostname running the process
    let full_hostname = hostname::get().expect("Failed to get hostname");
    let lossy_hostname = full_hostname.to_string_lossy();
    let hostname = lossy_hostname
        .split('.')
        .next()
        .expect("Unable to parse short hostname");
    info!("hostname: {}", hostname);

    // return the application Context object to the caller
    Context {
        config: datamove_config.clone(),
        db_pool,
        hostname: hostname.to_string(),
    }
}
