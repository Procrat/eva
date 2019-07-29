use std::io;

use chrono::prelude::*;
use chrono::Duration;
use diesel::prelude::*;
use futures::future;
use futures::future::LocalFutureObj;

use super::Database;
use super::{Error, Result};
use crate::time_segment::{
    NamedTimeSegment as CrateTimeSegment, NewNamedTimeSegment as CrateNewTimeSegment,
};

use self::tasks::dsl::tasks as task_table;
use self::time_segment_ranges::dsl::time_segment_ranges as time_segment_range_table;
use self::time_segments::dsl::time_segments as time_segment_table;

pub struct DbConnection(SqliteConnection);

#[derive(Debug, Clone, PartialEq, Queryable, Identifiable, AsChangeset, Associations)]
#[belongs_to(TimeSegment)]
#[table_name = "tasks"]
struct Task {
    pub id: i32,
    pub content: String,
    pub deadline: i32,
    pub duration: i32,
    pub importance: i32,
    pub time_segment_id: i32,
}

#[derive(Debug, Insertable)]
#[table_name = "tasks"]
struct NewTask {
    pub content: String,
    pub deadline: i32,
    pub duration: i32,
    pub importance: i32,
    pub time_segment_id: i32,
}

table! {
    tasks (id) {
        id -> Integer,
        content -> Text,
        deadline -> Integer,
        duration -> Integer,
        importance -> Integer,
        time_segment_id -> Integer,
    }
}

#[derive(Debug, Queryable, Identifiable, AsChangeset)]
#[table_name = "time_segments"]
struct TimeSegment {
    pub id: i32,
    pub name: String,
    pub start: i32,
    pub period: i32,
    pub hue: i32,
}

#[derive(Debug, Insertable)]
#[table_name = "time_segments"]
struct NewTimeSegment {
    pub name: String,
    pub start: i32,
    pub period: i32,
    pub hue: i32,
}

table! {
    time_segments (id) {
        id -> Integer,
        name -> VarChar,
        start -> Integer,
        period -> Integer,
        hue -> Integer,
    }
}

#[derive(Debug, Insertable, Queryable, Identifiable, Associations)]
#[belongs_to(TimeSegment, foreign_key = "segment_id")]
#[table_name = "time_segment_ranges"]
#[primary_key(start)]
struct TimeSegmentRange {
    pub segment_id: i32,
    pub start: i32,
    pub end: i32,
}

table! {
    time_segment_ranges (start) {
        segment_id -> Integer,
        start -> Integer,
        end -> Integer,
    }
}

embed_migrations!();

no_arg_sql_function!(last_insert_rowid, diesel::sql_types::Integer);

impl Database for DbConnection {
    fn add_task<'a: 'b, 'b>(
        &'a self,
        task: crate::NewTask,
    ) -> LocalFutureObj<'b, Result<crate::Task>> {
        let future_task = async move {
            diesel::insert_into(task_table)
                .values(&NewTask::from(task))
                .execute(&self.0)
                .map_err(|e| Error("while trying to add a task", e.into()))?;
            let id = diesel::select(last_insert_rowid)
                .get_result::<i32>(&self.0)
                .map_err(|e| Error("while trying to fetch the id of the new task", e.into()))?;
            let task = await!(self.get_task(id as u32))
                .map_err(|e| Error("while trying to fetch the newly created task", e.into()))?;
            Ok(task)
        };
        LocalFutureObj::new(Box::new(future_task))
    }

    fn delete_task<'a: 'b, 'b>(&'a self, id: u32) -> LocalFutureObj<'b, Result<()>> {
        let future = async move {
            let amount_deleted = diesel::delete(task_table.find(id as i32))
                .execute(&self.0)
                .map_err(|e| Error("while trying to delete a task", e.into()))?;
            if amount_deleted != 1 {
                return Err(Error(
                    "while trying to delete a task",
                    failure::format_err!("{} task(s) were deleted", amount_deleted),
                ));
            }
            Ok(())
        };
        LocalFutureObj::new(Box::new(future))
    }

    fn get_task<'a: 'b, 'b>(&'a self, id: u32) -> LocalFutureObj<'b, Result<crate::Task>> {
        let task_result = try {
            let db_task = task_table
                .find(id as i32)
                .get_result::<Task>(&self.0)
                .map_err(|e| Error("while trying to find a task", e.into()))?;
            crate::Task::from(db_task)
        };
        LocalFutureObj::new(Box::new(future::ready(task_result)))
    }

    fn update_task<'a: 'b, 'b>(&'a self, task: crate::Task) -> LocalFutureObj<'b, Result<()>> {
        let db_task = Task::from(task);
        let future = async move {
            let amount_updated = diesel::update(&db_task)
                .set(&db_task)
                .execute(&self.0)
                .map_err(|e| Error("while trying to update a task", e.into()))?;
            if amount_updated != 1 {
                return Err(Error(
                    "while trying to update a task",
                    failure::format_err!("{} task(s) were updated", amount_updated),
                ));
            }
            Ok(())
        };
        LocalFutureObj::new(Box::new(future))
    }

    fn all_tasks<'a: 'b, 'b>(&'a self) -> LocalFutureObj<'b, Result<Vec<crate::Task>>> {
        let tasks_result = try {
            let db_tasks = task_table
                .load::<Task>(&self.0)
                .map_err(|e| Error("while trying to retrieve tasks", e.into()))?;
            db_tasks.into_iter().map(crate::Task::from).collect()
        };
        LocalFutureObj::new(Box::new(future::ready(tasks_result)))
    }

    fn all_tasks_per_time_segment<'a: 'b, 'b>(
        &'a self,
    ) -> LocalFutureObj<'b, Result<Vec<(CrateTimeSegment, Vec<crate::Task>)>>> {
        let tasks_result = try {
            let db_time_segments = time_segments::table
                .load::<TimeSegment>(&self.0)
                .map_err(|e| Error("while trying to retrieve time segments", e.into()))?;
            let tasks = Task::belonging_to(&db_time_segments)
                .load::<Task>(&self.0)
                .map_err(|e| Error("while trying to retrieve tasks", e.into()))?
                .grouped_by(&db_time_segments)
                .into_iter()
                .map(|db_tasks| db_tasks.into_iter().map(crate::Task::from).collect());
            self.construct_time_segments(db_time_segments)?
                .zip(tasks)
                .collect()
        };
        LocalFutureObj::new(Box::new(future::ready(tasks_result)))
    }

    fn add_time_segment<'a: 'b, 'b>(
        &'a self,
        time_segment: CrateNewTimeSegment,
    ) -> LocalFutureObj<'b, Result<()>> {
        let result = try {
            diesel::insert_into(time_segment_table)
                .values(&NewTimeSegment::from(time_segment.clone()))
                .execute(&self.0)
                .map_err(|e| Error("while trying to add a time segment", e.into()))?;
            let id = diesel::select(last_insert_rowid)
                .get_result::<i32>(&self.0)
                .map_err(|e| Error("while trying to fetch the new time segment", e.into()))?;
            for range in time_segment.ranges {
                diesel::insert_into(time_segment_range_table)
                    .values(&TimeSegmentRange {
                        segment_id: id,
                        start: range.start.timestamp() as i32,
                        end: range.end.timestamp() as i32,
                    })
                    .execute(&self.0)
                    .map_err(|e| Error("while trying to add a time segment", e.into()))?;
            }
        };
        LocalFutureObj::new(Box::new(future::ready(result)))
    }

    fn delete_time_segment<'a: 'b, 'b>(
        &'a self,
        time_segment: CrateTimeSegment,
    ) -> LocalFutureObj<'b, Result<()>> {
        let db_time_segment = TimeSegment::from(time_segment);
        let ranges = TimeSegmentRange::belonging_to(&db_time_segment);
        let result = try {
            // Assert that there are no tasks in this time segment
            let n_tasks = Task::belonging_to(&db_time_segment)
                .count()
                .get_result::<i64>(&self.0)
                .map_err(|e| Error("while trying to delete a time segment", e.into()))?;
            if n_tasks > 0 {
                Err(Error(
                    "while trying to delete a time segment",
                    failure::format_err!(
                        "There are still {} task(s) in this time segment. Please move them to \
                         another time segment or delete them before deleting this segment.",
                        n_tasks
                    ),
                ))?
            }

            // Assert that this isn't the last time segment
            let n_time_segments = time_segments::table
                .count()
                .get_result::<i64>(&self.0)
                .map_err(|e| Error("while trying to count time segments", e.into()))?;
            if n_time_segments <= 1 {
                Err(Error(
                    "while trying to delete a time segment",
                    failure::format_err!(
                        "If you remove the last time segment, when should I schedule things?"
                    ),
                ))?
            }

            diesel::delete(ranges)
                .execute(&self.0)
                .map_err(|e| Error("while trying to delete a time segment", e.into()))?;
            let amount_deleted = diesel::delete(&db_time_segment)
                .execute(&self.0)
                .map_err(|e| Error("while trying to delete a time segment", e.into()))?;
            if amount_deleted != 1 {
                Err(Error(
                    "while trying to delete a time segment",
                    failure::format_err!("{} time segment(s) were deleted", amount_deleted),
                ))?
            }
        };
        LocalFutureObj::new(Box::new(future::ready(result)))
    }

    fn update_time_segment<'a: 'b, 'b>(
        &'a self,
        time_segment: CrateTimeSegment,
    ) -> LocalFutureObj<'b, Result<()>> {
        let db_time_segment = TimeSegment::from(time_segment.clone());
        let ranges = TimeSegmentRange::belonging_to(&db_time_segment);
        let result = try {
            diesel::delete(ranges)
                .execute(&self.0)
                .map_err(|e| Error("while trying to update a time segment", e.into()))?;
            for range in time_segment.ranges {
                diesel::insert_into(time_segment_range_table)
                    .values(&TimeSegmentRange {
                        segment_id: time_segment.id as i32,
                        start: range.start.timestamp() as i32,
                        end: range.end.timestamp() as i32,
                    })
                    .execute(&self.0)
                    .map_err(|e| Error("while trying to update a time segment", e.into()))?;
            }
            let amount_updated = diesel::update(&db_time_segment)
                .set(&db_time_segment)
                .execute(&self.0)
                .map_err(|e| Error("while trying to update a time segment", e.into()))?;
            if amount_updated != 1 {
                Err(Error(
                    "while trying to update a time segment",
                    failure::format_err!("{} time segment(s) were updated", amount_updated),
                ))?
            }
        };
        LocalFutureObj::new(Box::new(future::ready(result)))
    }

    fn all_time_segments<'a: 'b, 'b>(
        &'a self,
    ) -> LocalFutureObj<'b, Result<Vec<CrateTimeSegment>>> {
        let time_segments_result = try {
            let db_time_segments = time_segments::table
                .load::<TimeSegment>(&self.0)
                .map_err(|e| Error("while trying to retrieve time segments", e.into()))?;
            self.construct_time_segments(db_time_segments)?.collect()
        };
        LocalFutureObj::new(Box::new(future::ready(time_segments_result)))
    }
}

impl DbConnection {
    fn construct_time_segments(
        &self,
        db_time_segments: Vec<TimeSegment>,
    ) -> Result<impl Iterator<Item = CrateTimeSegment>> {
        let ranges = TimeSegmentRange::belonging_to(&db_time_segments)
            .load::<TimeSegmentRange>(&self.0)
            .map_err(|e| Error("while trying to retrieve time segments", e.into()))?
            .grouped_by(&db_time_segments)
            .into_iter()
            .map(|ranges| {
                ranges
                    .into_iter()
                    .map(|range| i32_to_datetime(range.start)..i32_to_datetime(range.end))
            });
        Ok(db_time_segments
            .into_iter()
            .zip(ranges)
            .map(|(segment, ranges)| CrateTimeSegment {
                id: segment.id as u32,
                name: segment.name,
                ranges: ranges.collect(),
                start: i32_to_datetime(segment.start),
                period: i32_to_duration(segment.period),
                hue: segment.hue as u16,
            }))
    }
}

impl From<crate::NewTask> for NewTask {
    fn from(task: crate::NewTask) -> NewTask {
        NewTask {
            content: task.content,
            deadline: task.deadline.timestamp() as i32,
            duration: task.duration.num_seconds() as i32,
            importance: task.importance as i32,
            time_segment_id: task.time_segment_id as i32,
        }
    }
}

impl From<Task> for crate::Task {
    fn from(task: Task) -> crate::Task {
        crate::Task {
            id: task.id as u32,
            content: task.content,
            deadline: i32_to_datetime(task.deadline),
            duration: i32_to_duration(task.duration),
            importance: task.importance as u32,
            time_segment_id: task.time_segment_id as u32,
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
            time_segment_id: task.time_segment_id as i32,
        }
    }
}

impl From<CrateNewTimeSegment> for NewTimeSegment {
    fn from(time_segment: CrateNewTimeSegment) -> NewTimeSegment {
        NewTimeSegment {
            name: time_segment.name,
            start: time_segment.start.timestamp() as i32,
            period: time_segment.period.num_seconds() as i32,
            hue: time_segment.hue as i32,
        }
    }
}

impl From<CrateTimeSegment> for TimeSegment {
    fn from(time_segment: CrateTimeSegment) -> TimeSegment {
        TimeSegment {
            id: time_segment.id as i32,
            name: time_segment.name,
            start: time_segment.start.timestamp() as i32,
            period: time_segment.period.num_seconds() as i32,
            hue: time_segment.hue as i32,
        }
    }
}

pub fn make_connection(database_url: &str) -> Result<DbConnection> {
    let connection = SqliteConnection::establish(database_url)
        .map_err(|e| Error("while trying to connect to the database", e.into()))?;
    // TODO run instead of run_with_output
    embedded_migrations::run_with_output(&connection, &mut io::stderr())
        .map_err(|e| Error("while running migrations", e.into()))?;
    Ok(DbConnection(connection))
}

fn i32_to_duration(duration: i32) -> Duration {
    Duration::seconds(i64::from(duration))
}

fn i32_to_datetime(timestamp: i32) -> DateTime<Utc> {
    let naive_datetime = NaiveDateTime::from_timestamp(i64::from(timestamp), 0);
    Utc.from_utc_datetime(&naive_datetime)
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
        assert_eq!(tasks[0], new_task);
        let same_task = block_on(connection.get_task(tasks[0].id)).unwrap();
        assert_eq!(tasks[0], same_task);

        // Deleting a task leaves the database empty
        block_on(connection.delete_task(tasks[0].id)).unwrap();
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
            &NaiveDateTime::parse_from_str("2015-09-05 23:56:04", "%Y-%m-%d %H:%M:%S").unwrap(),
        );
        task.content = "stuff".to_string();
        task.deadline = deadline;
        task.duration = Duration::minutes(7);
        task.importance = 100;
        block_on(connection.update_task(task.clone())).unwrap();

        let task_from_db = block_on(connection.get_task(task.id)).unwrap();
        assert_eq!(task, task_from_db);
    }

    #[test]
    fn test_default_time_segment() {
        let connection = make_connection(":memory:").unwrap();

        let mut time_segments = block_on(connection.all_time_segments()).unwrap();
        assert_eq!(time_segments.len(), 1);
        let time_segment = time_segments.pop().unwrap();
        assert_eq!(time_segment.id, 0);
        assert_eq!(time_segment.name, "Default");
        assert_eq!(time_segment.ranges.len(), 1);
        assert_eq!(time_segment.period, Duration::days(1));
        assert_eq!(time_segment.start, time_segment.ranges[0].start);
        assert_eq!(
            time_segment
                .start
                .with_timezone(&Local)
                .format("%H:%M:%S")
                .to_string(),
            "09:00:00"
        );
        assert_eq!(
            time_segment.ranges[0].end - time_segment.ranges[0].start,
            Duration::hours(8)
        );
        assert!(time_segment.hue < 360);

        // We shouldn't be able to delete the last time segment
        let result = block_on(connection.delete_time_segment(time_segment));
        assert_eq!(
            result.unwrap_err().to_string(),
            "A database error occurred while trying to delete a time segment: If you remove the \
             last time segment, when should I schedule things?"
        );
    }

    #[test]
    fn test_insert_query_and_delete_time_segment() {
        let connection = make_connection(":memory:").unwrap();

        let time_segment = test_time_segment();
        block_on(connection.add_time_segment(time_segment.clone())).unwrap();

        // There should be two segments now, the default and the one we added
        let mut time_segments = block_on(connection.all_time_segments()).unwrap();
        assert_eq!(time_segments.len(), 2);
        assert_eq!(time_segments[0].name, "Default");
        assert_eq!(time_segments[1], time_segment);

        // We should be able to query a task we add to a certain segment
        let mut task = test_task();
        task.time_segment_id = 1;
        let added_task = block_on(connection.add_task(task.clone())).unwrap();
        let tasks_per_segment = block_on(connection.all_tasks_per_time_segment()).unwrap();
        assert_eq!(tasks_per_segment.len(), 2);
        assert_eq!(tasks_per_segment[0].0.name, "Default");
        assert!(tasks_per_segment[0].1.is_empty());
        assert_eq!(tasks_per_segment[1].0, time_segment);
        assert_eq!(tasks_per_segment[1].1, [task]);

        // We shouldn't be able to delete the segment because there's still a
        // task in it
        let time_segment = time_segments.pop().unwrap();
        let result = block_on(connection.delete_time_segment(time_segment.clone()));
        let error_message = format!("{}", result.unwrap_err());
        assert_eq!(
            error_message,
            "A database error occurred while trying to delete a time segment: There are still 1 \
             task(s) in this time segment. Please move them to another time segment or delete \
             them before deleting this segment."
                .to_string()
        );
        let time_segments = block_on(connection.all_time_segments()).unwrap();
        assert_eq!(time_segments.len(), 2);

        // Once we delete the task, we should also be able to delete the segment
        block_on(connection.delete_task(added_task.id)).unwrap();
        block_on(connection.delete_time_segment(time_segment)).unwrap();
        let time_segments = block_on(connection.all_time_segments()).unwrap();
        assert_eq!(time_segments.len(), 1);
        assert_eq!(time_segments[0].name, "Default");
    }

    #[test]
    fn test_insert_update_query_time_segment() {
        let connection = make_connection(":memory:").unwrap();

        block_on(connection.add_time_segment(test_time_segment())).unwrap();

        let mut time_segment = block_on(connection.all_time_segments())
            .unwrap()
            .pop()
            .unwrap();
        time_segment.name = "changed name".to_string();
        let start = Utc::now().with_nanosecond(0).unwrap() + Duration::days(1);
        time_segment.start = start;
        time_segment.ranges = vec![start..start + Duration::minutes(3)];
        time_segment.period = Duration::minutes(42);
        time_segment.hue = 200;
        block_on(connection.update_time_segment(time_segment.clone())).unwrap();

        let time_segment_from_db = block_on(connection.all_time_segments())
            .unwrap()
            .pop()
            .unwrap();
        assert_eq!(time_segment_from_db, time_segment);
    }

    fn test_task() -> crate::NewTask {
        crate::NewTask {
            content: "do me".to_string(),
            deadline: Utc::now().with_nanosecond(0).unwrap(),
            duration: Duration::seconds(6),
            importance: 42,
            time_segment_id: 0,
        }
    }

    fn test_time_segment() -> CrateNewTimeSegment {
        let start = Utc::now().with_nanosecond(0).unwrap();
        CrateNewTimeSegment {
            name: "2h weekly".to_string(),
            ranges: vec![start..start + Duration::hours(2)],
            start,
            period: Duration::weeks(1),
            hue: 0,
        }
    }
}
