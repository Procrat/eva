#![feature(box_patterns)]

#[cfg(feature = "sqlite")]
#[macro_use]
extern crate diesel;
#[cfg(feature = "sqlite")]
#[macro_use]
extern crate diesel_migrations;

#[cfg(test)]
#[macro_use]
extern crate assert_matches;

use chrono::prelude::*;
use chrono::Duration;
use derive_new::new;
use failure::Fail;

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

pub async fn add_task(configuration: &Configuration, new_task: NewTask) -> Result<Task> {
    configuration
        .database
        .add_task(new_task)
        .await
        .map_err(Error::Database)
}

pub async fn delete_task(configuration: &Configuration, id: u32) -> Result<()> {
    configuration
        .database
        .delete_task(id)
        .await
        .map_err(Error::Database)
}

pub async fn get_task(configuration: &Configuration, id: u32) -> Result<Task> {
    configuration
        .database
        .get_task(id)
        .await
        .map_err(Error::Database)
}

pub async fn update_task(configuration: &Configuration, task: Task) -> Result<()> {
    configuration
        .database
        .update_task(task)
        .await
        .map_err(Error::Database)
}

pub async fn tasks(configuration: &Configuration) -> Result<Vec<Task>> {
    configuration
        .database
        .all_tasks()
        .await
        .map_err(Error::Database)
}

pub async fn schedule(configuration: &Configuration, strategy: &str) -> Result<Schedule<Task>> {
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
        .await
        .map_err(Error::Database)
        .and_then(move |tasks_per_segment| {
            Schedule::schedule(start, tasks_per_segment, strategy).map_err(Error::Schedule)
        })
}

pub async fn add_time_segment(
    configuration: &Configuration,
    time_segment: time_segment::NewNamedTimeSegment,
) -> Result<()> {
    configuration
        .database
        .add_time_segment(time_segment)
        .await
        .map_err(Error::Database)
}

pub async fn delete_time_segment(
    configuration: &Configuration,
    time_segment: time_segment::NamedTimeSegment,
) -> Result<()> {
    configuration
        .database
        .delete_time_segment(time_segment)
        .await
        .map_err(Error::Database)
}

pub async fn update_time_segment(
    configuration: &Configuration,
    time_segment: time_segment::NamedTimeSegment,
) -> Result<()> {
    configuration
        .database
        .update_time_segment(time_segment)
        .await
        .map_err(Error::Database)
}

pub async fn time_segments(
    configuration: &Configuration,
) -> Result<Vec<time_segment::NamedTimeSegment>> {
    configuration
        .database
        .all_time_segments()
        .await
        .map_err(Error::Database)
}
