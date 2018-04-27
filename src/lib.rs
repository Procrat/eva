#![feature(box_patterns)]

extern crate chrono;
#[macro_use]
extern crate derive_new;
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;
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

use configuration::Configuration;

pub use errors::{Error, ErrorKind, Result, ResultExt};
pub use scheduling::{Schedule, ScheduledTask};
use database::Database;

#[macro_use]
mod util;

pub mod configuration;
mod database;
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
    let database = default_database(configuration)?;
    database.add_task(new_task)
}

pub fn remove(configuration: &Configuration, id: u32) -> Result<()> {
    let database = default_database(configuration)?;
    database.remove_task(id)
}

pub fn get(configuration: &Configuration, id: u32) -> Result<Task> {
    let database = default_database(configuration)?;
    database.find_task(id)
}

pub fn update(configuration: &Configuration, task: Task) -> Result<()> {
    let database = default_database(configuration)?;
    database.update_task(task)
}

pub fn all(configuration: &Configuration) -> Result<Vec<Task>> {
    let database = default_database(configuration)?;
    database.all_tasks()
}

pub fn schedule(configuration: &Configuration, strategy: &str) -> Result<Schedule> {
    assert!(["importance", "urgency"].contains(&strategy));

    let database = default_database(configuration)?;
    let tasks = database.all_tasks()?;
    let schedule = match strategy {
        "importance" => Schedule::schedule_according_to_importance(tasks),
        "urgency" => Schedule::schedule_according_to_myrjam(tasks),
        _ => unreachable!(),
    }?;
    Ok(schedule)
}

fn default_database(configuration: &Configuration) -> Result<impl Database> {
    database::sqlite::make_connection(configuration)
}
