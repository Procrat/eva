use clap::{App, AppSettings, Arg, ArgMatches, SubCommand};
use eva::configuration::Configuration;
use failure::Fail;
use futures::executor::block_on;
use itertools::Itertools;

use crate::pretty_print::PrettyPrint;

mod configuration;
mod parse;
mod pretty_print;

#[derive(Debug, Fail)]
enum Error {
    #[fail(display = "{}", _0)]
    Configuration(#[cause] configuration::Error),
    #[fail(display = "{}", _0)]
    Parse(#[cause] parse::Error),
    #[fail(display = "{}", _0)]
    Eva(#[cause] eva::Error),
}

impl From<configuration::Error> for Error {
    fn from(error: configuration::Error) -> Error {
        Error::Configuration(error)
    }
}

impl From<parse::Error> for Error {
    fn from(error: parse::Error) -> Error {
        Error::Parse(error)
    }
}

impl From<eva::Error> for Error {
    fn from(error: eva::Error) -> Error {
        Error::Eva(error)
    }
}

type Result<T> = std::result::Result<T, Error>;

fn main() {
    if let Err(ref error) = run() {
        handle_error(error);
    }
}

fn run() -> Result<()> {
    let configuration = configuration::read()?;
    let matches = cli(&configuration).get_matches();
    dispatch(&matches, &configuration)
}

fn cli<'a, 'b>(configuration: &Configuration) -> App<'a, 'b> {
    let add = SubCommand::with_name("add")
        .about("Adds a task")
        .arg(Arg::with_name("content").required(true)
             .help("What is it that you want to do?"))
        .arg(Arg::with_name("deadline").required(true)
             .help("When should it be finished? \
                   Give it in the format of '2 Aug 2017 14:03'."))
        .arg(Arg::with_name("duration").required(true)
             .help("How long do you estimate it will take? \
                   Give it in a (whole or decimal) number of hours."))
        .arg(Arg::with_name("importance").required(true)
             .help("How important is this task to you on a scale from 1 to 10?"));
    let rm = SubCommand::with_name("rm")
        .about("Removes a task")
        .arg(Arg::with_name("task-id").required(true));
    let set = SubCommand::with_name("set")
        .about("Changes the deadline, duration, importance or content of an existing task")
        .arg(Arg::with_name("property").required(true)
             .possible_values(&["content", "deadline", "duration", "importance"]))
        .arg(Arg::with_name("task-id").required(true))
        .arg(Arg::with_name("value").required(true));
    let list = SubCommand::with_name("tasks")
        .about("Lists your tasks in the order you added them");
    let schedule = SubCommand::with_name("schedule")
        .about("Lets Eva suggest a schedule for your tasks")
        .arg(Arg::with_name("strategy")
             .long("strategy")
             .takes_value(true)
             .possible_values(&["importance", "urgency"])
             .default_value(configuration.scheduling_strategy.as_str()));

    App::new("eva")
        .version(env!("CARGO_PKG_VERSION"))
        .global_setting(AppSettings::ColoredHelp)
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommand(add)
        .subcommand(rm)
        .subcommand(set)
        .subcommand(list)
        .subcommand(schedule)
}

fn dispatch(inputs: &ArgMatches, configuration: &Configuration) -> Result<()> {
    match inputs.subcommand() {
        ("add", Some(submatches)) => {
            let content = submatches.value_of("content").unwrap();
            let deadline = submatches.value_of("deadline").unwrap();
            let duration = submatches.value_of("duration").unwrap();
            let importance = submatches.value_of("importance").unwrap();
            let new_task = eva::NewTask {
                content: content.to_owned(),
                deadline: parse::deadline(deadline)?,
                duration: parse::duration(duration)?,
                importance: parse::importance(importance)?,
                time_segment_id: 0,
            };
            let _task = block_on(eva::add(configuration, new_task))?;
            Ok(())
        }
        ("rm", Some(submatches)) => {
            let id = submatches.value_of("task-id").unwrap();
            let id = parse::id(id)?;
            Ok(block_on(eva::remove(configuration, id))?)
        }
        ("set", Some(submatches)) => {
            let field = submatches.value_of("property").unwrap();
            let id = submatches.value_of("task-id").unwrap();
            let value = submatches.value_of("value").unwrap();
            let id = parse::id(id)?;
            Ok(set_field(configuration, field, id, value)?)
        }
        ("tasks", Some(_submatches)) => {
            let tasks = block_on(eva::all(configuration))?;
            println!("Tasks:");
            for task in &tasks {
                // Indent all lines of task.pretty_print() by two spaces
                println!("  {}", task.pretty_print().split("\n").join("\n  "));
            }
            Ok(())
        }
        ("schedule", Some(submatches)) => {
            let strategy = submatches.value_of("strategy").unwrap().to_owned();
            let schedule = block_on(eva::schedule(configuration, &strategy))?;
            println!("{}", schedule.pretty_print());
            Ok(())
        }
        _ => unreachable!(),
    }
}

fn set_field(configuration: &Configuration, field: &str, id: u32, value: &str) -> Result<()> {
    let mut task = block_on(eva::get(configuration, id))?;
    match field {
        "content" => task.content = value.to_string(),
        "deadline" => task.deadline = parse::deadline(value)?,
        "duration" => task.duration = parse::duration(value)?,
        "importance" => task.importance = parse::importance(value)?,
        _ => unreachable!(),
    };
    Ok(block_on(eva::update(configuration, task))?)
}

fn handle_error(error: &Error) {
    eprintln!("{}", error);

    // Print backtrace when RUST_BACKTRACE=1
    if let Some(backtrace) = error.backtrace() {
        eprintln!("{:?}", backtrace);
    }

    ::std::process::exit(1);
}
