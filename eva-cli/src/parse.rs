use chrono::prelude::*;
use chrono::Duration;
use failure::Fail;

#[derive(Debug, Fail)]
#[fail(
    display = "I don't understand the {} you gave ({}). {}",
    type_, input, suggestion
)]
pub struct Error {
    type_: String,
    input: String,
    suggestion: String,
}

type Result<T> = std::result::Result<T, Error>;

pub fn id(id_str: &str) -> Result<u32> {
    id_str.parse::<u32>().map_err(|_| Error {
        type_: "id".to_owned(),
        input: id_str.to_owned(),
        suggestion: "Try entering a valid integer.".to_owned(),
    })
}

pub fn importance(importance_str: &str) -> Result<u32> {
    importance_str.parse::<u32>().map_err(|_| Error {
        type_: "importance".to_owned(),
        input: importance_str.to_owned(),
        suggestion: "Try entering a valid integer.".to_owned(),
    })
}

pub fn duration(duration_hours: &str) -> Result<Duration> {
    let hours = duration_hours.parse::<f64>().map_err(|_| Error {
        type_: "duration".to_owned(),
        input: duration_hours.to_owned(),
        suggestion: "Try entering a valid, real number.".to_owned(),
    })?;

    if hours <= 0.0 {
        return Err(Error {
            type_: "duration".to_owned(),
            input: duration_hours.to_owned(),
            suggestion: "Try entering a positive number.".to_owned(),
        });
    }

    Ok(Duration::minutes((60.0 * hours) as i64))
}

pub fn deadline(datetime: &str) -> Result<DateTime<Utc>> {
    Local
        .datetime_from_str(datetime, "%-d %b %Y %-H:%M")
        .map_err(|_| Error {
            type_: "deadline".to_owned(),
            input: datetime.to_owned(),
            suggestion: "Try entering something like '4 Jul 2017 6:05'.".to_owned(),
        })
        .map(|local_datetime| local_datetime.with_timezone(&Utc))
}
