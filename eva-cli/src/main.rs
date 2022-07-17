use std::env;
use std::process;

use anyhow::{Error, Result};
use clap::{builder::PossibleValuesParser, Arg, ArgMatches, Command};
use eva::configuration::Configuration;
use futures_executor::block_on;
use itertools::Itertools;

use crate::pretty_print::PrettyPrint;

mod configuration;
mod parse;
mod pretty_print;

fn main() {
    if let Err(error) = run() {
        handle_error(&error);
    }
}

fn run() -> Result<()> {
    let configuration = configuration::read()?;
    let arguments = cli(&configuration).get_matches();
    dispatch(&arguments, &configuration)
}

fn cli(configuration: &Configuration) -> Command {
    let add = Command::new("add")
        .about("Adds a task")
        .arg(
            Arg::new("content")
                .required(true)
                .help("What is it that you want to do?"),
        )
        .arg(Arg::new("deadline").required(true).help(
            "When should it be finished? \
                   Give it in the format of '2 Aug 2017 14:03'.",
        ))
        .arg(Arg::new("duration").required(true).help(
            "How long do you estimate it will take? \
                   Give it in a (whole or decimal) number of hours.",
        ))
        .arg(
            Arg::new("importance")
                .required(true)
                .help("How important is this task to you on a scale from 1 to 10?"),
        );
    let rm = Command::new("rm")
        .about("Removes a task")
        .arg(Arg::new("task-id").required(true));
    let set = Command::new("set")
        .about("Changes the deadline, duration, importance or content of an existing task")
        .arg(
            Arg::new("property")
                .required(true)
                .value_parser(PossibleValuesParser::new([
                    "content",
                    "deadline",
                    "duration",
                    "importance",
                ])),
        )
        .arg(Arg::new("task-id").required(true))
        .arg(Arg::new("value").required(true));
    let list = Command::new("tasks").about("Lists your tasks in the order you added them");
    let schedule = Command::new("schedule")
        .about("Lets Eva suggest a schedule for your tasks")
        .arg(
            Arg::new("strategy")
                .long("strategy")
                .takes_value(true)
                .value_parser(PossibleValuesParser::new(["importance", "urgency"]))
                .default_value(configuration.scheduling_strategy.as_str()),
        );

    Command::new("eva")
        .version(env!("CARGO_PKG_VERSION"))
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommands([add, rm, set, list, schedule])
}

fn dispatch(inputs: &ArgMatches, configuration: &Configuration) -> Result<()> {
    match inputs.subcommand().unwrap() {
        ("add", submatches) => {
            let content = submatches.get_one::<String>("content").unwrap();
            let deadline = submatches.get_one::<String>("deadline").unwrap();
            let duration = submatches.get_one::<String>("duration").unwrap();
            let importance = submatches.get_one::<String>("importance").unwrap();
            let new_task = eva::NewTask {
                content: content.to_owned(),
                deadline: parse::deadline(deadline)?,
                duration: parse::duration(duration)?,
                importance: parse::importance(importance)?,
                time_segment_id: 0,
            };
            let _task = block_on(eva::add_task(configuration, new_task))?;
            Ok(())
        }
        ("rm", submatches) => {
            let id = submatches.get_one::<String>("task-id").unwrap();
            let id = parse::id(id)?;
            Ok(block_on(eva::delete_task(configuration, id))?)
        }
        ("set", submatches) => {
            let field = submatches.get_one::<String>("property").unwrap();
            let id = submatches.get_one::<String>("task-id").unwrap();
            let value = submatches.get_one::<String>("value").unwrap();
            let id = parse::id(id)?;
            Ok(set_field(configuration, field, id, value)?)
        }
        ("tasks", _submatches) => {
            let tasks = block_on(eva::tasks(configuration))?;
            if tasks.len() == 0 {
                println!("No tasks left. Add one with `eva add`.");
            } else {
                println!("Tasks:");
                for task in &tasks {
                    // Indent all lines of task.pretty_print() by two spaces
                    println!("  {}", task.pretty_print().split("\n").join("\n  "));
                }
            }
            Ok(())
        }
        ("schedule", submatches) => {
            let strategy = submatches.get_one::<String>("strategy").unwrap().to_owned();
            let schedule = block_on(eva::schedule(configuration, &strategy))?;
            println!("{}", schedule.pretty_print());
            Ok(())
        }
        _ => unreachable!(),
    }
}

fn set_field(configuration: &Configuration, field: &str, id: u32, value: &str) -> Result<()> {
    let mut task = block_on(eva::get_task(configuration, id))?;
    match field {
        "content" => task.content = value.to_string(),
        "deadline" => task.deadline = parse::deadline(value)?,
        "duration" => task.duration = parse::duration(value)?,
        "importance" => task.importance = parse::importance(value)?,
        _ => unreachable!(),
    };
    Ok(block_on(eva::update_task(configuration, task))?)
}

fn handle_error(error: &Error) {
    eprintln!("{error}");

    if env::var("RUST_BACKTRACE").map_or(false, |v| v == "1") {
        eprintln!("\n{}", error.backtrace());
    }

    process::exit(1);
}
