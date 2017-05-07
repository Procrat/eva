use std::io;

use chrono::{DateTime, Duration, UTC, NaiveDateTime};
use diesel::prelude::*;
use diesel::types::{FromSql, Integer, HasSqlType, Text};
use diesel::backend::Backend;
use diesel::query_source::Queryable;
use diesel::sqlite::SqliteConnection;

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
    let connection = SqliteConnection::establish(DATABASE_URL)
        .expect(&format!("Error connecting to {}", DATABASE_URL));
    embedded_migrations::run_with_output(&connection, &mut io::stdout()).unwrap();
    return connection;
}

impl<DB> Queryable<(Integer, Text, Integer, Integer, Integer), DB> for Task
    where DB: Backend + HasSqlType<Integer> + HasSqlType<Text>,
          i32: FromSql<Integer, DB>,
          String: FromSql<Text, DB>,
{
    type Row = (i32, String, i32, i32, i32);

    fn build(row: Self::Row) -> Task {
        let naive_deadline = NaiveDateTime::from_timestamp(row.2 as i64, 0);
        let deadline = DateTime::from_utc(naive_deadline, UTC);
        let duration = Duration::seconds(row.3 as i64);
        Task {
            id: row.0 as u32,
            content: row.1,
            deadline: deadline,
            duration: duration,
            importance: row.4 as u32,
        }
    }
}
