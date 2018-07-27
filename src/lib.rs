#![feature(box_patterns)]

extern crate chrono;
#[macro_use]
extern crate derive_new;
#[macro_use]
extern crate error_chain;
extern crate itertools;
#[macro_use]
extern crate lazy_static;
extern crate take_mut;

#[cfg(feature = "sqlite")]
#[macro_use]
extern crate diesel;
#[cfg(feature = "sqlite")]
#[macro_use]
extern crate diesel_migrations;

#[cfg(test)]
#[macro_use]
extern crate assert_matches;


use std::hash::{Hash, Hasher};

use chrono::prelude::*;
use chrono::Duration;

use configuration::Configuration;

pub use errors::{Error, ErrorKind, Result, ResultExt};
pub use scheduling::{Schedule, ScheduledTask};
#[macro_use]
mod util;

pub mod configuration;
pub mod database;
mod scheduling;

pub mod errors {
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

#[derive(Debug, new, Clone)]
pub struct NewTask {
    pub content: String,
    pub deadline: DateTime<Local>,
    pub duration: Duration,
    pub importance: u32,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Task {
    pub id: u32,
    pub content: String,
    pub deadline: DateTime<Local>,
    pub duration: Duration,
    pub importance: u32,
}


// Hack because chrono::Duration doesn't implement the Hash trait.
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


pub fn add(configuration: &Configuration, new_task: NewTask) -> Result<Task> {
    configuration.database.add_task(new_task)
}

pub fn remove(configuration: &Configuration, id: u32) -> Result<()> {
    configuration.database.remove_task(id)
}

pub fn get(configuration: &Configuration, id: u32) -> Result<Task> {
    configuration.database.find_task(id)
}

pub fn update(configuration: &Configuration, task: Task) -> Result<()> {
    configuration.database.update_task(task)
}

pub fn all(configuration: &Configuration) -> Result<Vec<Task>> {
    configuration.database.all_tasks()
}

pub fn schedule(configuration: &Configuration, strategy: &str) -> Result<Schedule> {
    assert!(["importance", "urgency"].contains(&strategy));

    let tasks = configuration.database.all_tasks()?;
    let start = configuration.time_context.as_ref()
        .map_or_else(|| Local::now(), |time_context| time_context.now());
    let schedule = match strategy {
        "importance" => Schedule::schedule_according_to_importance(start, tasks),
        "urgency" => Schedule::schedule_according_to_myrjam(start, tasks),
        _ => unreachable!(),
    }?;
    Ok(schedule)
}
