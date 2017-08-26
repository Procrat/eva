use std::fs;
use std::path::{Path, PathBuf};

use app_dirs;
use app_dirs::{AppDataType, AppInfo};
use config;
use shellexpand;

const APP_INFO: AppInfo = AppInfo { name: "eva", author: "Stijn Seghers" };

lazy_static! {
    static ref CONFIG_ROOT: PathBuf = app_dirs::get_app_root(AppDataType::UserConfig, &APP_INFO)
        .unwrap_or_else(|err| {
            panic!(format!(
                    "An error occured while trying to find the configuration directory: {}",
                    err));
        });
    static ref DATA_ROOT: PathBuf = app_dirs::get_app_root(AppDataType::UserData, &APP_INFO)
        .unwrap_or_else(|err| {
            panic!(format!(
                    "An error occured while trying to find the local data directory: {}",
                    err));
        });
}


pub fn read() -> config::Config {
    let config_filename = CONFIG_ROOT.join("eva");
    let config_filename = config_filename.to_str().unwrap();

    let mut settings = config::Config::new();

    set_defaults(&mut settings)
        .merge(config::File::with_name(config_filename).required(false))
        .unwrap_or_else(|err| {
            panic!(format!("An error occured while reading local configuration file {}.toml: {}",
                           config_filename,
                           err));
        })
        .merge(config::Environment::with_prefix("eva"))
        .unwrap_or_else(|err| {
            panic!(format!("An error occured while reading environment variables: {}", err));
        });

    expand(&mut settings);

    ensure_paths_exist(&settings);

    settings
}


fn set_defaults(settings: &mut config::Config) -> &mut config::Config {
    let db_filename = DATA_ROOT.join("db.sqlite");
    let db_filename = db_filename.to_str().unwrap();

    settings
        .set_default("scheduling_strategy", "importance")
        .unwrap_or_else(|err| {
            panic!(format!("An error occured while setting configuration defaults: {}", err));
        })
        .set_default("database", db_filename)
        .unwrap_or_else(|err| {
            panic!(format!("An error occured while setting configuration defaults: {}", err));
        })
}


fn expand(settings: &mut config::Config) {
    let database_filename = settings.get_str("database")
        .unwrap_or_else(|err| {
            panic!(format!("An error occured while trying to read database path: {}", err));
        });
    let expanded_database_filename = shellexpand::full(&database_filename)
        .unwrap_or_else(|err| {
            panic!(format!("An error occured while expanding Eva's configuration: {}", err));
        });
    if expanded_database_filename != database_filename {
        settings.set("database", expanded_database_filename.to_string())
            .unwrap_or_else(|err| {
                panic!(format!("An error occured while setting exanded configuration: {}", err));
            });
    }
}


fn ensure_paths_exist(settings: &config::Config) {
    let database_filename = settings.get_str("database")
        .unwrap_or_else(|err| {
            panic!(format!("An error occured while trying to read database path: {}", err))
        });
    fs::create_dir_all(Path::new(&database_filename).parent().unwrap())
        .unwrap_or_else(|err| {
            panic!(format!("An error occured while trying to create database directory: {}", err));
        })
}
