use std::io;

use chrono::prelude::*;
use chrono::Duration;
use diesel::prelude::*;
use futures::future;
use futures::future::LocalFutureObj;

use crate::errors::*;
use super::Database;

use self::tasks::dsl::tasks as task_table;


#[derive(Debug, Clone, PartialEq, Queryable, Identifiable, AsChangeset)]
#[table_name="tasks"]
struct Task {
    pub id: i32,
    pub content: String,
    pub deadline: i32,
    pub duration: i32,
    pub importance: i32,
}

#[derive(Debug, Insertable)]
#[table_name="tasks"]
struct NewTask {
    pub content: String,
    pub deadline: i32,
    pub duration: i32,
    pub importance: i32,
}


table! {
    tasks (id) {
        id -> Integer,
        content -> Text,
        deadline -> Integer,
        duration -> Integer,
        importance -> Integer,
    }
}

embed_migrations!();

no_arg_sql_function!(last_insert_rowid, diesel::sql_types::Integer);


impl Database for SqliteConnection {
    fn add_task<'a: 'b, 'b>(&'a self, task: crate::NewTask) -> LocalFutureObj<'b, Result<crate::Task>> {
        let future_task = async move {
            diesel::insert_into(task_table)
                .values(&NewTask::from(task))
                .execute(self)
                .chain_err(|| ErrorKind::Database(
                    "while trying to add a task".into()))?;
            let id = diesel::select(last_insert_rowid)
                .get_result::<i32>(self)
                .chain_err(|| ErrorKind::Database(
                    "while trying to fetch the id of the new task".into()))?;
            let task = await!(self.find_task(id as u32))
                .chain_err(|| ErrorKind::Database(
                    "while trying to fetch the newly created task".into()))?;
            Ok(task)
        };
        LocalFutureObj::new(Box::new(future_task))
    }

    fn remove_task<'a: 'b, 'b>(&'a self, id: u32) -> LocalFutureObj<'b, Result<()>> {
        let future = async move {
            let amount_deleted = diesel::delete(task_table.find(id as i32))
                .execute(self)
                .chain_err(|| ErrorKind::Database("while trying to remove a task".to_owned()))?;
            ensure!(amount_deleted == 1,
                    ErrorKind::Database("while trying to remove a task".to_owned()));
            Ok(())
        };
        LocalFutureObj::new(Box::new(future))
    }

    fn find_task<'a: 'b, 'b>(&'a self, id: u32) -> LocalFutureObj<'b, Result<crate::Task>> {
        let task_result = try {
            let db_task = task_table.find(id as i32)
                .get_result::<Task>(self)
                .chain_err(|| ErrorKind::Database("while trying to find a task".to_owned()))?;
            crate::Task::from(db_task)
        };
        LocalFutureObj::new(Box::new(future::ready(task_result)))
    }

    fn update_task<'a: 'b, 'b>(&'a self, task: crate::Task) -> LocalFutureObj<'b, Result<()>> {
        let db_task = Task::from(task);
        let future = async move {
            let amount_updated = diesel::update(&db_task)
                .set(&db_task)
                .execute(self)
                .chain_err(|| ErrorKind::Database("while trying to update a task".to_owned()))?;
            ensure!(amount_updated == 1,
                    ErrorKind::Database("while trying to remove a task".to_owned()));
            Ok(())
        };
        LocalFutureObj::new(Box::new(future))
    }

    fn all_tasks<'a: 'b, 'b>(&'a self) -> LocalFutureObj<'b, Result<Vec<crate::Task>>> {
        let tasks_result = try {
            let db_tasks = task_table.load::<Task>(self)
                .chain_err(|| ErrorKind::Database("while trying to retrieve tasks".to_owned()))?;
            db_tasks.into_iter().map(crate::Task::from).collect()
        };
        LocalFutureObj::new(Box::new(future::ready(tasks_result)))
    }
}

impl From<crate::NewTask> for NewTask {
    fn from(task: crate::NewTask) -> NewTask {
        NewTask {
            content: task.content,
            deadline: task.deadline.timestamp() as i32,
            duration: task.duration.num_seconds() as i32,
            importance: task.importance as i32,
        }
    }
}

impl From<Task> for crate::Task {
    fn from(task: Task) -> crate::Task {
        let naive_deadline = NaiveDateTime::from_timestamp(i64::from(task.deadline), 0);
        let deadline = Utc.from_utc_datetime(&naive_deadline);
        let duration = Duration::seconds(i64::from(task.duration));
        crate::Task {
            id: task.id as u32,
            content: task.content,
            deadline: deadline,
            duration: duration,
            importance: task.importance as u32,
        }
    }
}

impl From<crate::Task> for Task {
    fn from(task: crate::Task) -> Task {
        Task {
            id: task.id as i32,
            content: task.content,
            deadline: task.deadline.timestamp() as i32,
            duration: task.duration.num_seconds() as i32,
            importance: task.importance as i32,
        }
    }
}


pub fn make_connection(database_url: &str) -> Result<SqliteConnection> {
    let connection = SqliteConnection::establish(database_url)
        .chain_err(|| ErrorKind::Database(format!("while trying to connect to {}", database_url)))?;
    // TODO run instead of run_with_output
    embedded_migrations::run_with_output(&connection, &mut io::stderr())
        .chain_err(|| ErrorKind::Database("while running migrations".to_owned()))?;
    Ok(connection)
}



#[cfg(test)]
mod tests {
    use futures::executor::block_on;

    use super::*;

    #[test]
    fn test_insert_query_and_delete_single_task() {
        let connection = make_connection(":memory:").unwrap();

        // Fresh database has no tasks
        assert_eq!(block_on(connection.all_tasks()).unwrap().len(), 0);

        // Inserting a task and querying for it, returns the same one
        let new_task = test_task();
        block_on(connection.add_task(new_task.clone())).unwrap();
        let tasks = block_on(connection.all_tasks()).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].content, new_task.content);
        assert_eq!(tasks[0].deadline.timestamp(), new_task.deadline.timestamp());
        assert_eq!(tasks[0].duration, new_task.duration);
        assert_eq!(tasks[0].importance, new_task.importance);
        let same_task = block_on(connection.find_task(tasks[0].id)).unwrap();
        assert_eq!(same_task.content, new_task.content);
        assert_eq!(same_task.deadline.timestamp(), new_task.deadline.timestamp());
        assert_eq!(same_task.duration, new_task.duration);
        assert_eq!(same_task.importance, new_task.importance);

        // Removing a task leaves the database empty
        block_on(connection.remove_task(tasks[0].id)).unwrap();
        assert!(block_on(connection.all_tasks()).unwrap().is_empty());
    }

    #[test]
    fn test_insert_update_query_single_task() {
        let connection = make_connection(":memory:").unwrap();

        let new_task = test_task();
        block_on(connection.add_task(new_task)).unwrap();

        let mut tasks = block_on(connection.all_tasks()).unwrap();
        let mut task = tasks.pop().unwrap();
        let deadline = Utc.from_utc_datetime(
            &NaiveDateTime::parse_from_str("2015-09-05 23:56:04", "%Y-%m-%d %H:%M:%S").unwrap());
        task.content = "stuff".to_string();
        task.deadline = deadline;
        task.duration = Duration::minutes(7);
        task.importance = 100;
        block_on(connection.update_task(task.clone())).unwrap();

        let task_from_db = block_on(connection.find_task(task.id)).unwrap();
        assert_eq!(task, task_from_db);
        assert_eq!(task.content, "stuff");
        assert_eq!(task.deadline, deadline);
        assert_eq!(task.duration, Duration::minutes(7));
        assert_eq!(task.importance, 100);
    }

    fn test_task() -> crate::NewTask {
        crate::NewTask {
            content: "do me".to_string(),
            deadline: Utc::now(),
            duration: Duration::seconds(6),
            importance: 42,
        }
    }
}
