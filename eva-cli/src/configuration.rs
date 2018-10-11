use std::fs;
use std::path::{Path, PathBuf};

use app_dirs;
use app_dirs::{AppDataType, AppInfo};
use config;
use eva;
use eva::configuration::{Configuration, SchedulingStrategy};
use shellexpand;

pub use self::errors::*;

mod errors {
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
            DatabaseConnect(path: String) {
                description("database connection error")
                display("I could not connect to the database ({})", path)
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


pub fn read() -> Result<Configuration> {
    let config_filename = config_root()?.join("eva.toml");
    let config_filename = config_filename.to_str()
        .ok_or_else(|| ErrorKind::FileCreation("my configuration directory".to_owned()))?;

    let mut configuration = config::Config::new();

    set_defaults(&mut configuration)?
        .merge(config::File::with_name(config_filename).required(false))
        .chain_err(|| ErrorKind::Read(format!("the local configuration file {}.toml",
                                              config_filename)))?
        .merge(config::Environment::with_prefix("eva"))
        .chain_err(|| ErrorKind::Read("environment variables".to_owned()))?;

    let database_path = configuration.get_str("database")
        .chain_err(|| ErrorKind::Read("the database path".to_owned()))?
        .expand("the database path")?;
    ensure_exists(&database_path, "the database path")?;
    let database = connect_to_database(&database_path)?;

    let scheduling_strategy = match
        configuration.get_str("scheduling_strategy")
        .chain_err(|| ErrorKind::Read("the scheduling strategy".to_owned()))?
        .as_str() {
            "importance" => SchedulingStrategy::Importance,
            "urgency" => SchedulingStrategy::Urgency,
            _ => bail!(ErrorKind::Read("the scheduling strategy".to_owned())),
        };

    Ok(Configuration {
        database: Box::new(database),
        scheduling_strategy: scheduling_strategy,
    })
}


fn config_root() -> Result<PathBuf> {
    app_dirs::get_app_root(AppDataType::UserConfig, &APP_INFO)
        .chain_err(|| ErrorKind::FileCreation("my configuration directory".to_owned()))
}


fn data_root() -> Result<PathBuf> {
    app_dirs::get_app_root(AppDataType::UserData, &APP_INFO)
        .chain_err(|| ErrorKind::FileCreation("my data directory".to_owned()))
}


fn set_defaults(configuration: &mut config::Config) -> Result<&mut config::Config> {
    let db_filename = data_root()?.join("db.sqlite");
    let db_filename = db_filename.to_str()
        .ok_or_else(|| ErrorKind::Default("the database path".to_owned()))?;

    Ok(configuration
        .set_default("scheduling_strategy", "importance")
        .chain_err(|| ErrorKind::Default("the scheduling strategy".to_owned()))?
        .set_default("database", db_filename)
        .chain_err(|| ErrorKind::Default("the database path".to_owned()))?
        )
}


trait ShellExpand {
    fn expand(&self, name: &str) -> Result<String>;
}

impl ShellExpand for String {
    fn expand(&self, name: &str) -> Result<String> {
        Ok(shellexpand::full(self)
           .chain_err(|| ErrorKind::ShellExpansion(name.to_owned()))?
           .into_owned())
    }
}


fn ensure_exists(path: &str, name: &str) -> Result<()> {
    let database_directory = Path::new(path).parent()
        .ok_or_else(|| ErrorKind::FileCreation(name.to_owned()))?;
    fs::create_dir_all(database_directory)
        .chain_err(|| ErrorKind::FileCreation(name.to_owned()))?;
    Ok(())
}


fn connect_to_database(path: &str) -> Result<impl eva::database::Database> {
    Ok(eva::database::sqlite::make_connection(path)
       .chain_err(|| ErrorKind::DatabaseConnect(path.to_owned()))?)
}
