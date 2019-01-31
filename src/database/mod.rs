use std::fmt;

use failure::Fail;
use futures::future::LocalFutureObj;

use crate::time_segment::NamedTimeSegment;
use crate::{NewTask, Task};

#[cfg(feature = "sqlite")]
pub mod sqlite;

#[derive(Debug, Fail)]
#[fail(display = "A database error occurred {}: {}", _0, _1)]
pub struct Error(pub &'static str, #[cause] pub failure::Error);

pub type Result<T> = std::result::Result<T, Error>;

pub trait Database {
    fn add_task<'a: 'b, 'b>(&'a self, task: NewTask) -> LocalFutureObj<'b, Result<Task>>;
    fn remove_task<'a: 'b, 'b>(&'a self, id: u32) -> LocalFutureObj<'b, Result<()>>;
    fn find_task<'a: 'b, 'b>(&'a self, id: u32) -> LocalFutureObj<'b, Result<Task>>;
    fn update_task<'a: 'b, 'b>(&'a self, task: Task) -> LocalFutureObj<'b, Result<()>>;
    fn all_tasks<'a: 'b, 'b>(&'a self) -> LocalFutureObj<'b, Result<Vec<Task>>>;
    fn all_tasks_per_time_segment<'a: 'b, 'b>(
        &'a self,
    ) -> LocalFutureObj<'b, Result<Vec<(NamedTimeSegment, Vec<Task>)>>>;
}

impl fmt::Debug for dyn Database {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<database connection>")
    }
}
