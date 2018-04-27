use std::io;

use chrono::prelude::*;
use chrono::Duration;
use diesel;
use diesel::prelude::*;

use ::errors::*;
use ::configuration::Configuration;
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
    fn add_task(&self, task: ::NewTask) -> Result<::Task> {
        diesel::insert_into(task_table)
            .values(&NewTask::from(task))
            .execute(self)
            .chain_err(|| ErrorKind::Database("while trying to add a task".to_owned()))?;
        let id: i32 = diesel::select(last_insert_rowid)
            .get_result(self)
            .chain_err(|| ErrorKind::Database("while trying to fetch the id of the new task".to_owned()))?;
        self.find_task(id as u32)
    }

    fn remove_task(&self, id: u32) -> Result<()> {
        let amount_deleted =
            diesel::delete(task_table.find(id as i32))
                .execute(self)
                .chain_err(|| ErrorKind::Database("while trying to remove a task".to_owned()))?;
        ensure!(amount_deleted == 1,
                ErrorKind::Database("while trying to remove a task".to_owned()));
        Ok(())
    }

    fn find_task(&self, id: u32) -> Result<::Task> {
        let db_task: Task = task_table.find(id as i32)
            .get_result(self)
            .chain_err(|| ErrorKind::Database("while trying to find a task".to_owned()))?;
        Ok(::Task::from(db_task))
    }

    fn update_task(&self, task: ::Task) -> Result<()> {
        let db_task = Task::from(task);
        let amount_updated =
            diesel::update(&db_task)
                .set(&db_task)
                .execute(self)
                .chain_err(|| ErrorKind::Database("while trying to update a task".to_owned()))?;
        ensure!(amount_updated == 1,
                ErrorKind::Database("while trying to remove a task".to_owned()));
        Ok(())
    }

    fn all_tasks(&self) -> Result<Vec<::Task>> {
        let db_tasks = task_table.load::<Task>(self)
            .chain_err(|| ErrorKind::Database("while trying to retrieve tasks".to_owned()))?;
        Ok(db_tasks.into_iter()
           .map(|task| ::Task::from(task))
           .collect())
    }
}

impl From<::NewTask> for NewTask {
    fn from(task: ::NewTask) -> NewTask {
        NewTask {
            content: task.content,
            deadline: task.deadline.timestamp() as i32,
            duration: task.duration.num_seconds() as i32,
            importance: task.importance as i32,
        }
    }
}

impl From<Task> for ::Task {
    fn from(task: Task) -> ::Task {
        let naive_deadline = NaiveDateTime::from_timestamp(i64::from(task.deadline), 0);
        let deadline = Local.from_utc_datetime(&naive_deadline);
        let duration = Duration::seconds(i64::from(task.duration));
        ::Task {
            id: task.id as u32,
            content: task.content,
            deadline: deadline,
            duration: duration,
            importance: task.importance as u32,
        }
    }
}

impl From<::Task> for Task {
    fn from(task: ::Task) -> Task {
        Task {
            id: task.id as i32,
            content: task.content,
            deadline: task.deadline.timestamp() as i32,
            duration: task.duration.num_seconds() as i32,
            importance: task.importance as i32,
        }
    }
}


pub fn make_connection(configuration: &Configuration) -> Result<SqliteConnection> {
    make_connection_with(&configuration.database_path)
}

fn make_connection_with(database_url: &str) -> Result<SqliteConnection> {
    let connection = SqliteConnection::establish(database_url)
        .chain_err(|| ErrorKind::Database(format!("while trying to connect to {}", database_url)))?;
    // TODO run instead of run_with_output
    embedded_migrations::run_with_output(&connection, &mut io::stderr())
        .chain_err(|| ErrorKind::Database("while running migrations".to_owned()))?;
    Ok(connection)
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_query_and_delete_single_task() {
        let connection = make_connection_with(":memory:").unwrap();

        // Fresh database has no tasks
        assert_eq!(connection.all_tasks().unwrap().len(), 0);

        // Inserting a task and querying for it, returns the same one
        let new_task = test_task();
        connection.add_task(new_task.clone()).unwrap();
        let tasks = connection.all_tasks().unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].content, new_task.content);
        assert_eq!(tasks[0].deadline.timestamp(), new_task.deadline.timestamp());
        assert_eq!(tasks[0].duration, new_task.duration);
        assert_eq!(tasks[0].importance, new_task.importance);
        let same_task = connection.find_task(tasks[0].id).unwrap();
        assert_eq!(same_task.content, new_task.content);
        assert_eq!(same_task.deadline.timestamp(), new_task.deadline.timestamp());
        assert_eq!(same_task.duration, new_task.duration);
        assert_eq!(same_task.importance, new_task.importance);

        // Removing a task leaves the database empty
        connection.remove_task(tasks[0].id).unwrap();
        assert!(connection.all_tasks().unwrap().is_empty());
    }

    #[test]
    fn test_insert_update_query_single_task() {
        let connection = make_connection_with(":memory:").unwrap();

        let new_task = test_task();
        connection.add_task(new_task).unwrap();

        let mut tasks = connection.all_tasks().unwrap();
        let mut task = tasks.pop().unwrap();
        let deadline = Local.from_utc_datetime(
            &NaiveDateTime::parse_from_str("2015-09-05 23:56:04", "%Y-%m-%d %H:%M:%S").unwrap());
        task.content = "stuff".to_string();
        task.deadline = deadline;
        task.duration = Duration::minutes(7);
        task.importance = 100;
        connection.update_task(task.clone()).unwrap();

        let task_from_db = connection.find_task(task.id).unwrap();
        assert_eq!(task, task_from_db);
        assert_eq!(task.content, "stuff");
        assert_eq!(task.deadline, deadline);
        assert_eq!(task.duration, Duration::minutes(7));
        assert_eq!(task.importance, 100);
    }

    fn test_task() -> ::NewTask {
        ::NewTask {
            content: "do me".to_string(),
            deadline: Local::now(),
            duration: Duration::seconds(6),
            importance: 42,
        }
    }
}
