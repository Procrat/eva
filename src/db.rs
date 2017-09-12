use std::io;

use chrono::{Duration, Local, NaiveDateTime, TimeZone};
use diesel::associations::HasTable;
use diesel::backend::Backend;
use diesel::expression::AsExpression;
use diesel::expression::helper_types::{AsExpr, Eq};
use diesel::insertable::ColumnInsertValue;
use diesel::prelude::*;
use diesel::query_builder::AsChangeset;
use diesel::query_builder::insert_statement::{InsertStatement, IntoInsertStatement};
use diesel::sqlite::{Sqlite, SqliteConnection};
use diesel::types::{FromSql, HasSqlType, Integer, Text};

use super::Task;
use configuration::Configuration;
use errors::*;


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


impl<DB> Queryable<(Integer, Text, Integer, Integer, Integer), DB> for Task
    where DB: Backend + HasSqlType<Integer> + HasSqlType<Text>,
          i32: FromSql<Integer, DB>,
          String: FromSql<Text, DB>
{
    type Row = (i32, String, i32, i32, i32);

    fn build(row: Self::Row) -> Task {
        let naive_deadline = NaiveDateTime::from_timestamp(i64::from(row.2), 0);
        let deadline = Local.from_utc_datetime(&naive_deadline);
        let duration = Duration::seconds(i64::from(row.3));
        Task {
            id: Some(row.0 as u32),
            content: row.1,
            deadline,
            duration,
            importance: row.4 as u32,
        }
    }
}


#[cfg_attr(feature = "cargo-clippy", allow(type_complexity))]
impl<'a> Insertable<tasks::table, Sqlite> for &'a Task {
    type Values = (ColumnInsertValue<tasks::content, AsExpr<String, tasks::content>>,
                   ColumnInsertValue<tasks::deadline, AsExpr<i32, tasks::deadline>>,
                   ColumnInsertValue<tasks::duration, AsExpr<i32, tasks::duration>>,
                   ColumnInsertValue<tasks::importance, AsExpr<i32, tasks::importance>>);

    fn values(self) -> Self::Values {
        (Insertable_column_expr!(tasks::content, self.content.clone(), regular),
         Insertable_column_expr!(tasks::deadline, self.deadline.timestamp() as i32, regular),
         Insertable_column_expr!(tasks::duration, self.duration.num_seconds() as i32, regular),
         Insertable_column_expr!(tasks::importance, self.importance as i32, regular))
    }
}

impl<'a, Op> IntoInsertStatement<tasks::table, Op> for &'a Task {
    type InsertStatement = InsertStatement<tasks::table, Self, Op>;

    fn into_insert_statement(self, target: tasks::table, operator: Op) -> Self::InsertStatement {
        InsertStatement::no_returning_clause(target, self, operator)
    }
}


impl<'a> Identifiable for &'a Task {
    type Id = i32;

    fn id(self) -> Self::Id {
        self.id.expect("Internal error: task id must not be None") as i32
    }
}

impl HasTable for Task {
    type Table = tasks::dsl::tasks;

    fn table() -> Self::Table {
        tasks::dsl::tasks
    }
}


#[cfg_attr(feature = "cargo-clippy", allow(type_complexity))]
impl AsChangeset for Task {
    type Target = tasks::dsl::tasks;
    type Changeset = (Eq<tasks::dsl::content, String>,
                      Eq<tasks::dsl::deadline, i32>,
                      Eq<tasks::dsl::duration, i32>,
                      Eq<tasks::dsl::importance, i32>,
                      );

    fn as_changeset(self) -> Self::Changeset {
        (tasks::dsl::content.eq(self.content),
         tasks::dsl::deadline.eq(self.deadline.timestamp() as i32),
         tasks::dsl::duration.eq(self.duration.num_seconds() as i32),
         tasks::dsl::importance.eq(self.importance as i32),
         )
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use chrono::prelude::*;
    use diesel;

    #[test]
    fn test_insert_query_and_delete_single_task() {
        use self::tasks::dsl::tasks;

        let connection = make_connection_with(":memory:").unwrap();
        let new_task = test_task();

        diesel::insert(&new_task)
            .into(tasks)
            .execute(&connection)
            .unwrap();

        let tasks_ = tasks.load::<Task>(&connection).unwrap();
        assert_eq!(tasks_, [new_task]);

        let id = tasks_[0].id.unwrap();

        diesel::delete(tasks.find(id as i32))
            .execute(&connection)
            .unwrap();

        let tasks_ = tasks.load::<Task>(&connection).unwrap();
        assert!(tasks_.is_empty());
    }

    #[test]
    fn test_insert_update_query_single_task() {
        use self::tasks::dsl::tasks;

        let connection = make_connection_with(":memory:").unwrap();
        let new_task = test_task();
        diesel::insert(&new_task)
            .into(tasks)
            .execute(&connection)
            .unwrap();

        let mut tasks_ = tasks.load::<Task>(&connection).unwrap();
        let mut task = tasks_.pop().unwrap();
        let deadline = Local.from_utc_datetime(
            &NaiveDateTime::parse_from_str("2015-09-05 23:56:04", "%Y-%m-%d %H:%M:%S").unwrap());
        task.content = "stuff".to_string();
        task.deadline = deadline;
        task.duration = Duration::minutes(7);
        task.importance = 100;
        let task2 = task.clone();

        diesel::update(&task)
            .set(task)
            .execute(&connection)
            .unwrap();

        let task_from_db = tasks.find(task2.id.unwrap() as i32).first(&connection).unwrap();
        assert_eq!(task2, task_from_db);
        assert_eq!(task2.content, "stuff");
        assert_eq!(task2.deadline, deadline);
        assert_eq!(task2.duration, Duration::minutes(7));
        assert_eq!(task2.importance, 100);
    }

    fn test_task() -> Task {
        Task {
            id: None,
            content: "do me".to_string(),
            deadline: Local::now().with_nanosecond(0).unwrap(),
            duration: Duration::seconds(6),
            importance: 42,
        }
    }
}
