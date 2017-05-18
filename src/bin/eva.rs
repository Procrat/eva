extern crate chrono;
extern crate clap;
extern crate eva;

use clap::{App, AppSettings, Arg, SubCommand};


fn cli<'a, 'b>() -> App<'a, 'b> {
    let add = SubCommand::with_name("add")
        .arg(Arg::with_name("content").required(true))
        .arg(Arg::with_name("deadline").required(true))
        .arg(Arg::with_name("duration").required(true))
        .arg(Arg::with_name("importance").required(true));
    let rm = SubCommand::with_name("rm")
        .arg(Arg::with_name("id").required(true));
    let schedule = SubCommand::with_name("schedule");

    return App::new("eva")
        .version(env!("CARGO_PKG_VERSION"))
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommand(add)
        .subcommand(rm)
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
            let duration: f64 = duration.parse()
                .expect("Please supply a valid (real) number as duration.");
            let importance: u32 = importance.parse()
                .expect("Please supply a valid integer as importance factor.");
            eva::add(content, deadline, duration, importance)
        },
        ("rm", Some(submatches)) => {
            let id = submatches.value_of("id").unwrap();
            let id: u32 = id.parse()
                .expect("Please supply a valid integer as id.");
            eva::remove(id)
        },
        ("schedule", Some(_submatches)) => {
            eva::print_schedule()
        },
        _ => unreachable!(),
    };
}
