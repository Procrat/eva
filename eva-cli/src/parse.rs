use chrono::prelude::*;
use chrono::Duration;


pub use self::errors::*;

mod errors {
    error_chain! {
        errors {
            Parse(type_: String, input: String, suggestion: String) {
                description("parse error")
                display("I don't understand the {} you gave ({}). {}",
                        type_, input, suggestion)
            }
        }
    }
}


pub fn id(id_str: &str) -> Result<u32> {
    id_str.parse()
        .chain_err(|| ErrorKind::Parse(
            "id".to_owned(),
            id_str.to_owned(),
            "Try entering a valid integer.".to_owned()))
}

pub fn importance(importance_str: &str) -> Result<u32> {
    importance_str.parse()
        .chain_err(|| ErrorKind::Parse(
            "importance".to_owned(),
            importance_str.to_owned(),
            "Try entering a valid integer.".to_owned()))
}

pub fn duration(duration_hours: &str) -> Result<Duration> {
    let hours: f64 = duration_hours.parse()
        .chain_err(|| ErrorKind::Parse(
            "duration".to_owned(),
            duration_hours.to_owned(),
            "Try entering a valid, real number.".to_owned()))?;

    ensure!(hours > 0.0, ErrorKind::Parse(
        "duration".to_owned(),
        duration_hours.to_owned(),
        "Try entering a positive number.".to_owned()));

    Ok(Duration::minutes((60.0 * hours) as i64))
}

pub fn deadline(datetime: &str) -> Result<DateTime<Utc>> {
    Local.datetime_from_str(datetime, "%-d %b %Y %-H:%M")
        .chain_err(|| ErrorKind::Parse(
            "deadline".to_owned(),
            datetime.to_owned(),
            "Try entering something like '4 Jul 2017 6:05'.".to_owned())
        )
        .map(|local_datetime| local_datetime.with_timezone(&Utc))
}
