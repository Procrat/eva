use std::io;

use chrono::{DateTime, Duration, NaiveDateTime, UTC};
use diesel::backend::Backend;
use diesel::expression::AsExpression;
use diesel::expression::helper_types::AsNullableExpr;
use diesel::insertable::ColumnInsertValue;
use diesel::prelude::*;
use diesel::query_builder::insert_statement::{InsertStatement, IntoInsertStatement};
use diesel::sqlite::{Sqlite, SqliteConnection};
use diesel::types::{FromSql, HasSqlType, Integer, Text};

use super::Task;


const DATABASE_URL: &'static str = "db.sqlite";

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


pub fn make_connection() -> SqliteConnection {
    make_connection_with(DATABASE_URL)
}

fn make_connection_with(database_url: &str) -> SqliteConnection {
    let connection = SqliteConnection::establish(database_url)
        .expect(&format!("Error connecting to {}", database_url));
    // TODO run instead of run_with_output + unwrap
    embedded_migrations::run_with_output(&connection, &mut io::stdout()).unwrap();
    connection
}


impl<DB> Queryable<(Integer, Text, Integer, Integer, Integer), DB> for Task
    where DB: Backend + HasSqlType<Integer> + HasSqlType<Text>,
          i32: FromSql<Integer, DB>,
          String: FromSql<Text, DB>
{
    type Row = (i32, String, i32, i32, i32);

    fn build(row: Self::Row) -> Task {
        let naive_deadline = NaiveDateTime::from_timestamp(row.2 as i64, 0);
        let deadline = DateTime::from_utc(naive_deadline, UTC);
        let duration = Duration::seconds(row.3 as i64);
        Task {
            id: Some(row.0 as u32),
            content: row.1,
            deadline: deadline,
            duration: duration,
            importance: row.4 as u32,
        }
    }
}


impl<'a> Insertable<tasks::table, Sqlite> for &'a Task {
    type Values = (ColumnInsertValue<tasks::content, AsNullableExpr<String, tasks::content>>,
     ColumnInsertValue<tasks::deadline, AsNullableExpr<i32, tasks::deadline>>,
     ColumnInsertValue<tasks::duration, AsNullableExpr<i32, tasks::duration>>,
     ColumnInsertValue<tasks::importance, AsNullableExpr<i32, tasks::importance>>);

    fn values(self) -> Self::Values {
        use diesel::types::IntoNullable;

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


#[cfg(test)]
mod tests {
    use super::*;
    use chrono::prelude::*;
    use diesel;

    #[test]
    fn test_insert_query_and_delete_single_task() {
        use self::tasks::dsl::tasks;

        let connection = make_connection_with(":memory:");

        let new_task = Task {
            id: None,
            content: "do me".to_string(),
            deadline: UTC::now().with_nanosecond(0).unwrap(),
            duration: Duration::seconds(6),
            importance: 42,
        };

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
}
