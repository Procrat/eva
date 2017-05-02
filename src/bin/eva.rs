extern crate chrono;
extern crate clap;
extern crate eva;

use clap::{App, AppSettings, Arg, SubCommand};


fn cli<'a, 'b>() -> App<'a, 'b> {
    let add = SubCommand::with_name("add")
        .arg(Arg::with_name("name").required(true));
    let rm = SubCommand::with_name("rm")
        .arg(Arg::with_name("name").required(true));
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
            let name = submatches.value_of("name").unwrap();
            eva::add(name)
        },
        ("rm", Some(submatches)) => {
            let name = submatches.value_of("name").unwrap();
            eva::remove(name)
        },
        ("schedule", Some(_submatches)) => {
            eva::print_schedule()
        },
        _ => unreachable!(),
    };
}
