extern crate chrono;
extern crate clap;
#[macro_use]
extern crate lazy_static;

extern crate eva;

use clap::{App, AppSettings, Arg, SubCommand};

lazy_static! {
    static ref DEFAULT_SCHEDULING_ALGORITHM: String = eva::CONFIG.get_str("scheduling_algorithm")
        .unwrap_or_else(|err| {
            panic!(format!("An error occured while reading the default algorithm: {}", err));
        });
}


fn cli<'a, 'b>() -> App<'a, 'b> {
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
    let schedule = SubCommand::with_name("schedule")
        .about("Lets Eva suggest a schedule for your tasks")
        .arg(Arg::with_name("algorithm")
             .long("algorithm")
             .takes_value(true)
             .possible_values(&["importance", "urgency"])
             .default_value(&DEFAULT_SCHEDULING_ALGORITHM));

    return App::new("eva")
        .version(env!("CARGO_PKG_VERSION"))
        .global_setting(AppSettings::ColoredHelp)
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommand(add)
        .subcommand(rm)
        .subcommand(set)
        .subcommand(schedule)
}

fn main() {
    let matches = cli().get_matches();

    match matches.subcommand() {
        ("add", Some(submatches)) => {
            let content = submatches.value_of("content").unwrap();
            let deadline = submatches.value_of("deadline").unwrap();
            let duration = submatches.value_of("duration").unwrap();
            let importance = submatches.value_of("importance").unwrap();
            let importance: u32 = importance.parse()
                .expect("Please supply a valid integer as importance factor.");
            eva::add(content, deadline, duration, importance)
        },
        ("rm", Some(submatches)) => {
            let id = submatches.value_of("task-id").unwrap();
            let id: u32 = id.parse()
                .expect("Please supply a valid integer as id.");
            eva::remove(id)
        },
        ("set", Some(submatches)) => {
            let field = submatches.value_of("property").unwrap();
            let id = submatches.value_of("task-id").unwrap();
            let value = submatches.value_of("value").unwrap();
            let id: u32 = id.parse()
                .expect("Please supply a valid integer as id.");
            eva::set(field, id, value);
        }
        ("schedule", Some(submatches)) => {
            let algorithm = submatches.value_of("algorithm").unwrap();
            eva::print_schedule(algorithm)
        },
        _ => unreachable!(),
    };
}
