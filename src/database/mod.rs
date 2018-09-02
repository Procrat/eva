use std::fmt;

use futures::future::LocalFutureObj;

use crate::{NewTask, Task};
use crate::errors::*;


#[cfg(feature = "sqlite")]
pub mod sqlite;


pub trait Database {
    fn add_task<'a: 'b, 'b>(&'a self, task: NewTask) -> LocalFutureObj<'b, Result<Task>>;
    fn remove_task<'a: 'b, 'b>(&'a self, id: u32) -> LocalFutureObj<'b, Result<()>>;
    fn find_task<'a: 'b, 'b>(&'a self, id: u32) -> LocalFutureObj<'b, Result<Task>>;
    fn update_task<'a: 'b, 'b>(&'a self, task: Task) -> LocalFutureObj<'b, Result<()>>;
    fn all_tasks<'a: 'b, 'b>(&'a self) -> LocalFutureObj<'b, Result<Vec<Task>>>;
}

impl fmt::Debug for Database {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<database connection>")
    }
}
