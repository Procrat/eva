use std::fs;
use std::path::{Path, PathBuf};

use app_dirs;
use app_dirs::{AppDataType, AppInfo};
use config;
use shellexpand;

use configuration::errors::*;

#[allow(unused_doc_comment)]
pub mod errors {
    error_chain! {
        errors {
            Read(what: String) {
                description("configuration parsing error")
                display("An error occurred while trying to read {}", what)
            }
            FileCreation(config_name: String) {
                description("file creation error while reading configuration")
                display("I could not create {}", config_name)
            }
            ShellExpansion(what: String) {
                description("shell expansion error while reading configuration")
                display("An error occurred while trying to expand the configuration of {}", what)
            }
            Default(what: String) {
                description("setting defaults error while reading configuration")
                display("An error occurred while trying to set the default configuration of {}",
                        what)
            }
        }
    }
}

const APP_INFO: AppInfo = AppInfo { name: "eva", author: "Stijn Seghers" };


pub fn read() -> Result<config::Config> {
    let config_filename = config_root()?.join("eva");
    let config_filename = config_filename.to_str()
        .ok_or_else(|| ErrorKind::FileCreation("my configuration directory".to_owned()))?;

    let mut settings = config::Config::new();

    set_defaults(&mut settings)?
        .merge(config::File::with_name(config_filename).required(false))
        .chain_err(|| ErrorKind::Read(format!("the local configuration file {}.toml",
                                              config_filename)))?
        .merge(config::Environment::with_prefix("eva"))
        .chain_err(|| ErrorKind::Read("environment variables".to_owned()))?;

    expand(&mut settings)?;

    ensure_paths_exist(&settings)?;

    Ok(settings)
}


fn config_root() -> Result<PathBuf> {
    app_dirs::get_app_root(AppDataType::UserConfig, &APP_INFO)
        .chain_err(|| ErrorKind::FileCreation("my configuration directory".to_owned()))
}


fn data_root() -> Result<PathBuf> {
    app_dirs::get_app_root(AppDataType::UserData, &APP_INFO)
        .chain_err(|| ErrorKind::FileCreation("my data directory".to_owned()))
}


fn set_defaults(settings: &mut config::Config) -> Result<&mut config::Config> {
    let db_filename = data_root()?.join("db.sqlite");
    let db_filename = db_filename.to_str()
        .ok_or_else(|| ErrorKind::Default("the database path".to_owned()))?;

    Ok(settings
        .set_default("scheduling_strategy", "importance")
        .chain_err(|| ErrorKind::Default("the scheduling strategy".to_owned()))?
        .set_default("database", db_filename)
        .chain_err(|| ErrorKind::Default("the database path".to_owned()))?
        )
}


fn expand(settings: &mut config::Config) -> Result<()> {
    let database_filename = settings.get_str("database")
        .chain_err(|| ErrorKind::Read("the database path".to_owned()))?;
    let expanded_database_filename = shellexpand::full(&database_filename)
        .chain_err(|| ErrorKind::ShellExpansion("the database path".to_owned()))?;
    if expanded_database_filename != database_filename {
        settings.set("database", expanded_database_filename.to_string())
            .chain_err(|| ErrorKind::ShellExpansion("the database path".to_owned()))?;
    }
    Ok(())
}


fn ensure_paths_exist(settings: &config::Config) -> Result<()> {
    let database_filename = settings.get_str("database")
        .chain_err(|| ErrorKind::Read("the database path".to_owned()))?;
    let database_directory = Path::new(&database_filename).parent()
        .ok_or_else(|| ErrorKind::FileCreation("the database directory".to_owned()))?;
    fs::create_dir_all(database_directory)
        .chain_err(|| ErrorKind::FileCreation("the database directory".to_owned()))?;
    Ok(())
}
