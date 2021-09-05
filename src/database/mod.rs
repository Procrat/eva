use std::fmt;

use async_trait::async_trait;
use failure::Fail;

use crate::time_segment::{NamedTimeSegment as TimeSegment, NewNamedTimeSegment as NewTimeSegment};
use crate::{NewTask, Task};

#[cfg(feature = "sqlite")]
pub mod sqlite;

#[derive(Debug, Fail)]
#[fail(display = "A database error occurred {}: {}", _0, _1)]
pub struct Error(pub &'static str, #[cause] pub failure::Error);

pub type Result<T> = std::result::Result<T, Error>;

#[async_trait(?Send)]
pub trait Database {
    async fn add_task(&self, task: NewTask) -> Result<Task>;
    async fn delete_task(&self, id: u32) -> Result<()>;
    async fn get_task(&self, id: u32) -> Result<Task>;
    async fn update_task(&self, task: Task) -> Result<()>;
    async fn all_tasks(&self) -> Result<Vec<Task>>;
    async fn all_tasks_per_time_segment(&self) -> Result<Vec<(TimeSegment, Vec<Task>)>>;

    async fn add_time_segment(&self, time_segment: NewTimeSegment) -> Result<()>;
    async fn delete_time_segment(&self, time_segment: TimeSegment) -> Result<()>;
    async fn update_time_segment(&self, time_segment: TimeSegment) -> Result<()>;
    async fn all_time_segments(&self) -> Result<Vec<TimeSegment>>;
}

impl fmt::Debug for dyn Database {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<database connection>")
    }
}
