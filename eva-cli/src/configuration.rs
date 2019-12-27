use std::fs;
use std::path::Path;

use config;
use directories::ProjectDirs;
use eva::configuration::{Configuration, SchedulingStrategy};
use failure::Fail;
use shellexpand;

#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "Unfortunately, only GNU/Linux, Mac OS and Windows are supported.")]
    UnsupportedOS(),
    #[fail(display = "An error occurred while trying to read {}: {}", _0, _1)]
    Read(&'static str, #[cause] failure::Error),
    #[fail(display = "I could not create {}: {}", _0, _1)]
    FileCreation(&'static str, #[cause] failure::Error),
    #[fail(
        display = "An error occurred while trying to expand the configuration of {}: {}",
        _0, _1
    )]
    ShellExpansion(String, #[cause] failure::Error),
    #[fail(display = "I could not connect to the database ({}): {}", _0, _1)]
    DatabaseConnect(String, #[cause] eva::database::Error),
    #[fail(
        display = "An error occurred while trying to set the default configuration of {}: {}",
        _0, _1
    )]
    Default(&'static str, #[cause] failure::Error),
}

type Result<T> = std::result::Result<T, Error>;

pub fn read() -> Result<Configuration> {
    let project_dirs = ProjectDirs::from("", "", "eva").ok_or_else(|| Error::UnsupportedOS())?;

    let config_filename = project_dirs.config_dir().join("eva.toml");
    let config_filename = config_filename.to_str().ok_or_else(|| {
        Error::FileCreation(
            "my configuration directory",
            failure::err_msg("The config directory path contains illegal characters"),
        )
    })?;

    let mut configuration = config::Config::new();

    set_defaults(&mut configuration, &project_dirs)?
        .merge(config::File::with_name(config_filename).required(false))
        .map_err(|e| Error::Read("the local configuration file", e.into()))?
        .merge(config::Environment::with_prefix("eva"))
        .map_err(|e| Error::Read("environment variables", e.into()))?;

    let database_path = configuration
        .get_str("database")
        .map_err(|e| Error::Read("the database path", e.into()))?
        .expand("the database path")?;
    ensure_exists(&database_path)
        .map_err(|e| Error::FileCreation("the database path", e.into()))?;
    let database = connect_to_database(&database_path)?;

    let scheduling_strategy = match configuration
        .get_str("scheduling_strategy")
        .map_err(|e| Error::Read("the scheduling strategy", e.into()))?
        .as_str()
    {
        "importance" => SchedulingStrategy::Importance,
        "urgency" => SchedulingStrategy::Urgency,
        _ => {
            return Err(Error::Read(
                "the scheduling strategy",
                failure::err_msg("The scheduling strategy must be `importance` or `urgency`"),
            ));
        }
    };

    Ok(Configuration {
        database: Box::new(database),
        scheduling_strategy,
    })
}

fn set_defaults<'a>(
    configuration: &'a mut config::Config,
    project_dirs: &ProjectDirs,
) -> Result<&'a mut config::Config> {
    let db_filename = project_dirs.data_dir().join("db.sqlite");
    let db_filename = db_filename.to_str().ok_or_else(|| {
        Error::Default(
            "the database path",
            failure::err_msg("The database directory path contains illegal characters"),
        )
    })?;

    Ok(configuration
        .set_default("scheduling_strategy", "importance")
        .map_err(|e| Error::Default("the scheduling strategy", e.into()))?
        .set_default("database", db_filename)
        .map_err(|e| Error::Default("the database path", e.into()))?)
}

trait ShellExpand {
    fn expand(&self, name: &str) -> Result<String>;
}

impl ShellExpand for String {
    fn expand(&self, name: &str) -> Result<String> {
        Ok(shellexpand::full(self)
            .map_err(|e| Error::ShellExpansion(name.into(), e.into()))?
            .into_owned())
    }
}

fn ensure_exists(path: &str) -> std::result::Result<(), failure::Error> {
    let database_directory = Path::new(path)
        .parent()
        .ok_or_else(|| failure::err_msg("A parent directory does not exist"))?;
    fs::create_dir_all(database_directory)?;
    Ok(())
}

fn connect_to_database(path: &str) -> Result<impl eva::database::Database> {
    Ok(eva::database::sqlite::make_connection(path)
        .map_err(|e| Error::DatabaseConnect(path.into(), e.into()))?)
}
