#![feature(box_patterns)]

extern crate chrono;
#[macro_use]
extern crate derive_new;
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_codegen;
#[macro_use]
extern crate error_chain;
extern crate itertools;
#[macro_use]
extern crate lazy_static;
extern crate take_mut;

#[cfg(test)]
#[macro_use]
extern crate assert_matches;

use std::hash::{Hash, Hasher};

use chrono::prelude::*;
use chrono::Duration;
use diesel::prelude::*;

use configuration::Configuration;

pub use errors::{Error, ErrorKind, Result, ResultExt};
pub use scheduling::{Schedule, ScheduledTask};

#[macro_use]
mod util;

pub mod configuration;
mod db;
mod scheduling;

#[allow(unused_doc_comment)]
mod errors {
    use scheduling;

    error_chain! {
        links {
            Schedule(scheduling::Error, scheduling::ErrorKind);
        }
        errors {
            Parse(what: String, how_it_should_be: String) {
                description("parse error")
                display("I could not parse the {}. {}", what, how_it_should_be)
            }
            Database(when: String) {
                description("database error")
                display("A database error occurred {}", when)
            }
            Internal(more_info: String) {
                description("internal error")
                display("An internal error occurred (This shouldn't happen.): {}", more_info)
            }
        }
    }
}


#[derive(Debug, Eq, new, Clone)]
pub struct Task {
    pub id: Option<u32>,
    pub content: String,
    pub deadline: DateTime<Local>,
    pub duration: Duration,
    pub importance: u32,
}

impl PartialEq for Task {
    fn eq(&self, other: &Self) -> bool {
        let equal_id = match (self.id, other.id) {
            (Some(id1), Some(id2)) => id1 == id2,
            _ => true,
        };
        equal_id &&
            self.content == other.content &&
            self.deadline == other.deadline &&
            self.duration == other.duration &&
            self.importance == other.importance
    }
}

// Hack because chrono::Duration, which is a re-export of std::time::Duration, does not re-export
// implementation of Hash trait.
impl Hash for Task {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.content.hash(state);
        self.deadline.hash(state);
        self.duration.to_std()
            .expect(&format!("Internal error: duration of {} was negative", self))
            .hash(state);
        self.importance.hash(state);
    }
}


pub fn add(configuration: &Configuration,
           content: &str,
           deadline: DateTime<Local>,
           duration: Duration,
           importance: u32)
    -> Result<()>
{
    use db::tasks::dsl::tasks;

    let connection = db::make_connection(configuration)?;

    let new_task = Task {
        id: None,
        content: content.to_string(),
        deadline,
        duration,
        importance,
    };

    diesel::insert(&new_task)
        .into(tasks)
        .execute(&connection)
        .chain_err(|| ErrorKind::Database("while trying to add a task".to_owned()))?;

    Ok(())
}

pub fn remove(configuration: &Configuration, id: u32) -> Result<()> {
    use db::tasks::dsl::tasks;

    let connection = db::make_connection(configuration)?;

    let amount_deleted =
        diesel::delete(tasks.find(id as i32))
        .execute(&connection)
        .chain_err(|| ErrorKind::Database("while trying to remove a task".to_owned()))?;

    if amount_deleted == 0 {
        bail!(ErrorKind::Database(format!("while trying to find the task with id {}", id)));
    } else if amount_deleted > 1 {
        bail!(ErrorKind::Internal("multiple tasks got deleted".to_owned()));
    }

    Ok(())
}

pub fn set(configuration: &Configuration, field_name: &str, id: u32, value: &str) -> Result<()> {
    assert!(["content", "deadline", "duration", "importance"].contains(&field_name));

    use db::tasks::dsl::tasks;

    let connection = db::make_connection(configuration)?;

    let mut task: Task = tasks.find(id as i32)
        .first(&connection)
        .chain_err(|| ErrorKind::Database("while trying to retrieve a task".to_owned()))?;

    match field_name {
        "content" => task.content = value.to_string(),
        "deadline" => task.deadline = parse_datetime(value)?,
        "duration" => task.duration = parse_duration(value)?,
        "importance" => task.importance = parse_importance(value)?,
        _ => unreachable!(),
    }

    let amount_updated = diesel::update(&task)
        .set(task)
        .execute(&connection)
        .chain_err(|| ErrorKind::Database("while trying to update a task".to_owned()))?;

    if amount_updated == 0 {
        bail!(ErrorKind::Database("while trying to update a task".to_owned()));
    } else if amount_updated > 1 {
        bail!(ErrorKind::Internal("multiple tasks got deleted".to_owned()));
    }

    Ok(())
}

pub fn list_tasks(configuration: &Configuration) -> Result<Vec<Task>> {
    use db::tasks::dsl::tasks;

    let connection = db::make_connection(configuration)?;

    Ok(tasks.load::<Task>(&connection)
        .chain_err(|| ErrorKind::Database("while trying to retrieve tasks".to_owned()))?)
}

pub fn schedule(configuration: &Configuration, strategy: &str) -> Result<Schedule> {
    assert!(["importance", "urgency"].contains(&strategy));

    use db::tasks::dsl::tasks;

    let connection = db::make_connection(configuration)?;

    let tasks_ = tasks.load::<Task>(&connection)
        .chain_err(|| ErrorKind::Database("while trying to retrieve tasks".to_owned()))?;

    let schedule = match strategy {
        "importance" => Schedule::schedule_according_to_importance(tasks_),
        "urgency" => Schedule::schedule_according_to_myrjam(tasks_),
        _ => unreachable!(),
    }?;

    Ok(schedule)
}


pub fn parse_datetime(datetime: &str) -> Result<DateTime<Local>> {
    Local.datetime_from_str(datetime, "%-d %b %Y %-H:%M")
        .chain_err(|| {
            ErrorKind::Parse("deadline".to_owned(),
                             "Please provide something like '4 Jul 2017 6:05'".to_owned())
        })
}

fn parse_importance(importance_str: &str) -> Result<u32> {
    importance_str.parse()
        .chain_err(|| ErrorKind::Parse("importance".to_owned(),
                                       "Please supply a valid integer".to_owned()))
}

pub fn parse_duration(duration_hours: &str) -> Result<Duration> {
    let hours: f64 = duration_hours.parse()
        .chain_err(|| ErrorKind::Parse("duration".to_owned(),
                                       "Please supply a valid, real number".to_owned()))?;
    if hours <= 0.0 {
        bail!(ErrorKind::Parse("duration".to_owned(),
                               "Please supply a positive number".to_owned()));
    }
    Ok(Duration::minutes((60.0 * hours) as i64))
}
