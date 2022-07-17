use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use directories::ProjectDirs;

use eva::configuration::{Configuration, SchedulingStrategy};

pub fn read() -> Result<Configuration> {
    let project_dirs = ProjectDirs::from("", "", "eva")
        .context("Unfortunately, only GNU/Linux, Mac OS and Windows are supported.")?;

    let config_filename = project_dirs.config_dir().join("eva.toml");
    let configuration = default_configuration(&project_dirs)?
        .add_source(config::File::from(config_filename).required(false))
        .add_source(config::Environment::with_prefix("eva"))
        .build()
        .context("I couldn't read the configuration settings")?;

    let database_path_raw = configuration
        .get_string("database")
        .context("I couldn't read the preferred database path")?;
    let database_path = shellexpand::tilde(&database_path_raw);
    ensure_exists(&database_path)
        .with_context(|| format!("I couldn't create the database path: {database_path}"))?;
    let database = connect_to_database(&database_path)?;

    let scheduling_strategy = match configuration
        .get_string("scheduling_strategy")
        .context("I couldn't read the preferred scheduling strategy")?
        .as_str()
    {
        "importance" => SchedulingStrategy::Importance,
        "urgency" => SchedulingStrategy::Urgency,
        _ => {
            anyhow::bail!("The scheduling strategy must be either set to `importance` or `urgency`")
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
    let db_filename = db_filename
        .to_str()
        .context("The database directory path contains illegal characters")?;

    Ok(configuration
        .set_default("scheduling_strategy", "importance")
        .expect("Failed to set default setting for scheduling strategy")
        .set_default("database", db_filename)
        .expect("Failed to set default setting for database path"))
}

fn ensure_exists(path: &str) -> Result<()> {
    let database_directory = Path::new(path).parent().with_context(|| {
        format!("The database path \"{path}\" does not have a parent directory")
    })?;
    fs::create_dir_all(database_directory)?;
    Ok(())
}

fn connect_to_database(path: &str) -> Result<impl eva::database::Database> {
    Ok(eva::database::sqlite::make_connection(path)
        .with_context(|| format!("I could not connect to the database ({path})"))?)
}
