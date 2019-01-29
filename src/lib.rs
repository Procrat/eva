#![feature(box_patterns)]
#![feature(futures_api)]
#![feature(async_await, await_macro)]
#![feature(try_blocks)]

#[cfg(feature = "sqlite")]
#[macro_use]
extern crate diesel;
#[cfg(feature = "sqlite")]
#[macro_use]
extern crate diesel_migrations;

use std::collections::HashMap;

use chrono::prelude::*;
use chrono::Duration;
use derive_new::new;
use failure::Fail;
use futures::prelude::*;

use crate::configuration::{Configuration, SchedulingStrategy};
use crate::time_segment::TimeSegment;

pub use crate::scheduling::{Schedule, ScheduledTask};

pub mod configuration;
pub mod database;
mod scheduling;
mod time_segment;
mod util;

#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "{}", _0)]
    Database(#[cause] crate::database::Error),
    #[fail(display = "{}", _0)]
    Schedule(#[cause] crate::scheduling::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, new, Clone)]
pub struct NewTask {
    pub content: String,
    pub deadline: DateTime<Utc>,
    pub duration: Duration,
    pub importance: u32,
}

#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub struct Task {
    pub id: u32,
    pub content: String,
    pub deadline: DateTime<Utc>,
    pub duration: Duration,
    pub importance: u32,
}

pub fn add<'a: 'b, 'b>(
    configuration: &'a Configuration,
    new_task: NewTask,
) -> impl Future<Output = Result<Task>> + 'b {
    configuration
        .database
        .add_task(new_task)
        .map_err(Error::Database)
}

pub fn remove<'a: 'b, 'b>(
    configuration: &'a Configuration,
    id: u32,
) -> impl Future<Output = Result<()>> + 'b {
    configuration
        .database
        .remove_task(id)
        .map_err(Error::Database)
}

pub fn get<'a: 'b, 'b>(
    configuration: &'a Configuration,
    id: u32,
) -> impl Future<Output = Result<Task>> + 'b {
    configuration
        .database
        .find_task(id)
        .map_err(Error::Database)
}

pub fn update<'a: 'b, 'b>(
    configuration: &'a Configuration,
    task: Task,
) -> impl Future<Output = Result<()>> + 'b {
    configuration
        .database
        .update_task(task)
        .map_err(Error::Database)
}

pub fn all<'a: 'b, 'b>(
    configuration: &'a Configuration,
) -> impl Future<Output = Result<Vec<Task>>> + 'b {
    configuration.database.all_tasks().map_err(Error::Database)
}

pub fn schedule<'a: 'c, 'b: 'c, 'c>(
    configuration: &'a Configuration,
    strategy: &'b str,
) -> impl Future<Output = Result<Schedule>> + 'c {
    let strategy = match strategy {
        "importance" => SchedulingStrategy::Importance,
        "urgency" => SchedulingStrategy::Urgency,
        _ => panic!("Unsupported scheduling strategy provided"),
    };
    // Ensure everything is scheduled for some time after the algorithm has
    // finished.
    let start = configuration.now() + Duration::minutes(1);

    configuration
        .database
        .all_tasks()
        .map_err(Error::Database)
        .and_then(move |tasks| {
            let mut tasks_per_segment = HashMap::new();
            let anytime = TimeSegment {
                ranges: vec![start..start + Duration::weeks(1)],
                start,
                period: Duration::weeks(1),
            };
            tasks_per_segment.insert(anytime, tasks);
            let schedule = Schedule::schedule(start, tasks_per_segment, strategy);
            future::ready(schedule).map_err(Error::Schedule)
        })
}
