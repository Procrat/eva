#![feature(box_patterns)]
#![feature(try_blocks)]

#[cfg(feature = "sqlite")]
#[macro_use]
extern crate diesel;
#[cfg(feature = "sqlite")]
#[macro_use]
extern crate diesel_migrations;

use chrono::prelude::*;
use chrono::Duration;
use derive_new::new;
use failure::Fail;
use futures::prelude::*;

use crate::configuration::{Configuration, SchedulingStrategy};

pub use crate::scheduling::{Schedule, Scheduled};

pub mod configuration;
pub mod database;
mod scheduling;
pub mod time_segment;
mod util;

#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "{}", _0)]
    Database(#[cause] crate::database::Error),
    #[fail(display = "{}", _0)]
    Schedule(#[cause] crate::scheduling::Error<Task>),
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, new, Clone)]
pub struct NewTask {
    pub content: String,
    pub deadline: DateTime<Utc>,
    pub duration: Duration,
    pub importance: u32,
    pub time_segment_id: u32,
}

#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub struct Task {
    pub id: u32,
    pub content: String,
    pub deadline: DateTime<Utc>,
    pub duration: Duration,
    pub importance: u32,
    pub time_segment_id: u32,
}

impl PartialEq<NewTask> for Task {
    fn eq(&self, other: &NewTask) -> bool {
        self.content == other.content
            && self.deadline == other.deadline
            && self.duration == other.duration
            && self.importance == other.importance
            && self.time_segment_id == other.time_segment_id
    }
}

pub fn add_task<'a: 'b, 'b>(
    configuration: &'a Configuration,
    new_task: NewTask,
) -> impl Future<Output = Result<Task>> + 'b {
    configuration
        .database
        .add_task(new_task)
        .map_err(Error::Database)
}

pub fn delete_task<'a: 'b, 'b>(
    configuration: &'a Configuration,
    id: u32,
) -> impl Future<Output = Result<()>> + 'b {
    configuration
        .database
        .delete_task(id)
        .map_err(Error::Database)
}

pub fn get_task<'a: 'b, 'b>(
    configuration: &'a Configuration,
    id: u32,
) -> impl Future<Output = Result<Task>> + 'b {
    configuration.database.get_task(id).map_err(Error::Database)
}

pub fn update_task<'a: 'b, 'b>(
    configuration: &'a Configuration,
    task: Task,
) -> impl Future<Output = Result<()>> + 'b {
    configuration
        .database
        .update_task(task)
        .map_err(Error::Database)
}

pub fn tasks<'a: 'b, 'b>(
    configuration: &'a Configuration,
) -> impl Future<Output = Result<Vec<Task>>> + 'b {
    configuration.database.all_tasks().map_err(Error::Database)
}

pub fn schedule<'a: 'c, 'b: 'c, 'c>(
    configuration: &'a Configuration,
    strategy: &'b str,
) -> impl Future<Output = Result<Schedule<Task>>> + 'c {
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
        .all_tasks_per_time_segment()
        .map_err(Error::Database)
        .and_then(move |tasks_per_segment| {
            let schedule = Schedule::schedule(start, tasks_per_segment, strategy);
            future::ready(schedule).map_err(Error::Schedule)
        })
}

pub fn add_time_segment<'a: 'b, 'b>(
    configuration: &'a Configuration,
    time_segment: time_segment::NewNamedTimeSegment,
) -> impl Future<Output = Result<()>> + 'b {
    configuration
        .database
        .add_time_segment(time_segment)
        .map_err(Error::Database)
}

pub fn delete_time_segment<'a: 'b, 'b>(
    configuration: &'a Configuration,
    time_segment: time_segment::NamedTimeSegment,
) -> impl Future<Output = Result<()>> + 'b {
    configuration
        .database
        .delete_time_segment(time_segment)
        .map_err(Error::Database)
}

pub fn update_time_segment<'a: 'b, 'b>(
    configuration: &'a Configuration,
    time_segment: time_segment::NamedTimeSegment,
) -> impl Future<Output = Result<()>> + 'b {
    configuration
        .database
        .update_time_segment(time_segment)
        .map_err(Error::Database)
}

pub fn time_segments<'a: 'b, 'b>(
    configuration: &'a Configuration,
) -> impl Future<Output = Result<Vec<time_segment::NamedTimeSegment>>> + 'b {
    configuration
        .database
        .all_time_segments()
        .map_err(Error::Database)
}
