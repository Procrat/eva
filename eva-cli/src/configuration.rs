use std::fs;
use std::path::Path;

use directories::ProjectDirs;
use eva::configuration::{Configuration, SchedulingStrategy};
use failure::Fail;

#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "Unfortunately, only GNU/Linux, Mac OS and Windows are supported.")]
    UnsupportedOS(),
    #[fail(display = "An error occurred while trying to read {}: {}", _0, _1)]
    Read(&'static str, #[cause] failure::Error),
    #[fail(display = "I could not create {}: {}", _0, _1)]
    FileCreation(&'static str, #[cause] failure::Error),
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
    let configuration = default_configuration(&project_dirs)?
        .add_source(config::File::from(config_filename).required(false))
        .add_source(config::Environment::with_prefix("eva"))
        .build()
        .map_err(|e| Error::Read("the configuration settings", e.into()))?;

    let database_path_raw = configuration
        .get_string("database")
        .map_err(|e| Error::Read("the database path", e.into()))?;
    let database_path = shellexpand::tilde(&database_path_raw);
    ensure_exists(&database_path)
        .map_err(|e| Error::FileCreation("the database path", e.into()))?;
    let database = connect_to_database(&database_path)?;

    let scheduling_strategy = match configuration
        .get_string("scheduling_strategy")
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

fn default_configuration(
    project_dirs: &ProjectDirs,
) -> Result<config::ConfigBuilder<config::builder::DefaultState>> {
    let configuration = config::Config::builder();

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
