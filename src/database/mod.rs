use ::{NewTask, Task};
use ::errors::Result;


pub mod sqlite;


pub trait Database {
    fn add_task(&self, task: NewTask) -> Result<Task>;
    fn remove_task(&self, id: u32) -> Result<()>;
    fn find_task(&self, id: u32) -> Result<Task>;
    fn update_task(&self, task: Task) -> Result<()>;
    fn all_tasks(&self) -> Result<Vec<Task>>;
}
