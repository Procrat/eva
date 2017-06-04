#![feature(box_patterns)]

#[macro_use]
extern crate derive_new;
extern crate chrono;
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_codegen;
extern crate itertools;
extern crate take_mut;

#[cfg(test)]
#[macro_use]
extern crate assert_matches;


use std::fmt;
use std::hash::{Hash, Hasher};

use chrono::prelude::*;
use chrono::Duration;
use diesel::prelude::*;
use itertools::Itertools;

use schedule_tree::ScheduleTree;

#[macro_use]
mod util;

mod db;
mod schedule_tree;


pub fn add(content: &str, deadline: &str, duration: &str, importance: u32) {
    use db::tasks::dsl::tasks;

    let connection = db::make_connection();

    let deadline = parse_datetime(deadline);
    let duration = parse_duration(duration);
    let new_task = Task {
        id: None,
        content: content.to_string(),
        deadline: deadline,
        duration: duration,
        importance: importance,
    };

    diesel::insert(&new_task)
        .into(tasks)
        .execute(&connection)
        .expect("Error saving task.");
}

pub fn remove(id: u32) {
    use db::tasks::dsl::tasks;

    let connection = db::make_connection();

    let amount_deleted =
        diesel::delete(tasks.find(id as i32))
        .execute(&connection)
        .expect("Error removing task.");

    if amount_deleted == 0 {
        panic!("Could not find task with id {}", id)
    } else if amount_deleted > 1 {
        panic!("Internal error (this should not happen): multiple tasks got deleted.")
    }
}

pub fn set(field_name: &str, id: u32, value: &str) {
    assert!(["content", "deadline", "duration", "importance"].contains(&field_name));

    use db::tasks::dsl::tasks;

    let connection = db::make_connection();

    let mut task: Task = tasks.find(id as i32)
        .first(&connection)
        .expect("Error retrieving task");

    match field_name {
        "content" => task.content = value.to_string(),
        "deadline" => task.deadline = parse_datetime(value),
        "duration" => task.duration = parse_duration(value),
        "importance" => task.importance = value.parse()
            .expect("Please supply a valid integer"),
        _ => unreachable!(),
    }

    let amount_updated = diesel::update(&task)
        .set(task)
        .execute(&connection)
        .expect("Error updating task.");

    if amount_updated == 0 {
        panic!("Could not update task.")
    } else if amount_updated > 1 {
        panic!("Internal error (this should not happen): multiple tasks got deleted.")
    }
}

pub fn print_schedule() {
    use db::tasks::dsl::tasks;

    let connection = db::make_connection();

    let tasks_ = tasks
        .load::<Task>(&connection)
        .expect("Error retrieving tasks.");

    println!("Tasks:");
    for task in &tasks_ {
        println!("  {}", task);
    }

    let schedule = Schedule::schedule(&tasks_);
    println!("\n{}", schedule);
}


#[derive(Debug, Eq, new, Clone)]
pub struct Task {
    id: Option<u32>,
    content: String,
    deadline: DateTime<UTC>,
    duration: Duration,
    importance: u32,
}

impl PartialEq for Task {
    fn eq(&self, other: &Self) -> bool {
        let equal_id = match (self.id, other.id) {
            (Some(id1), Some(id2)) => id1 == id2,
            _ => true,
        };
        equal_id &&
            self.content == other.content &&
            self.deadline == other.deadline &&
            self.duration == other.duration &&
            self.importance == other.importance
    }
}

// Hack because chrono::Duration, which is a re-export of std::time::Duration, does not re-export
// implementation of Hash trait.
impl Hash for Task {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.content.hash(state);
        self.deadline.hash(state);
        self.duration.to_std().unwrap().hash(state);
        self.importance.hash(state);
    }
}


#[derive(Debug, new)]
struct ScheduledTask<'a> {
    task: &'a Task,
    when: DateTime<UTC>,
}

#[derive(Debug)]
pub struct Schedule<'a>(Vec<ScheduledTask<'a>>);

impl<'a> Schedule<'a> {
    /// Schedules tasks according to their deadlines, importance and duration.
    ///
    /// First, all tasks --- starting with the most important until the least important --- are
    /// scheduled as close as possible to their deadline.
    /// Next, all tasks --- starting with the most urgent one until the least urgent --- are put as
    /// close to the present as possible. This is too take care of contingencies like falling sick.
    /// A downside might be that a lot of time is spent doing urgent but non-important tasks.
    ///
    /// Args:
    ///     tasks: ordered list of tasks to schedule, ordered from most important to least
    ///     important.
    /// Returns an instance of Schedule which contains all tasks, each bound to certain date and
    /// time.
    pub fn schedule<'b: 'a>(tasks: &'b [Task]) -> Schedule<'a> {
        let mut tree = ScheduleTree::new();
        for task in tasks {
            if ! tree.schedule_close_before(task.deadline, task.duration, Some(UTC::now()), task) {
                // TODO Figure out what should be done in this case
                panic!("Out of time! Not all tasks could be scheduled.")
            }
        }
        let importance_schedule = tree.iter()
            .map(|entry| ScheduledTask::new(entry.data, entry.start))
            .collect();
        // TODO let urgent_first_schedule = importance_schedule
        Schedule(importance_schedule)
    }
}

impl<'a> fmt::Display for Schedule<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "Schedule:\n  "));
        write!(f, "{}", self.0.iter().join("\n  "))
    }
}

impl<'a> fmt::Display for ScheduledTask<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {}",
               format_datetime(self.when),
               self.task)
    }
}

impl fmt::Display for Task {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let prefix = match self.id {
            Some(id) => format!("{}.", id),
            None => "- ".to_string(),
        };
        write!(f, "{} {}\n    (deadline: {}, duration: {}, importance: {})",
               prefix,
               self.content,
               format_datetime(self.deadline),
               format_duration(self.duration),
               self.importance)
    }
}

fn format_datetime(datetime: DateTime<UTC>) -> String {
    datetime.format("%a %-d %b %-H:%M").to_string()
}

fn format_duration(duration: Duration) -> String {
    if duration.num_minutes() > 0 {
        format!("{}h{}", duration.num_hours(), duration.num_minutes() % 60)
    } else {
        format!("{}h", duration.num_hours())
    }
}

fn parse_datetime(datetime: &str) -> DateTime<UTC> {
    UTC.datetime_from_str(datetime, "%-d %b %Y %-H:%M")
        .expect("Could not parse deadline. Please provide something like '4 Jul 2017 6:05'.")
}

fn parse_duration(duration_hours: &str) -> Duration {
    let hours: f64 = duration_hours.parse()
        .expect("Please supply a valid real number as duration.");
    Duration::minutes((60.0 * hours) as i64)
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_tasks_are_planned() {
        let tasks = taskset1();
        let schedule = Schedule::schedule(&tasks);
        for scheduled_task in schedule.0 {
            assert!(tasks.contains(&scheduled_task.task))
        }
    }

    #[test]
    fn tasks_are_in_order() {
        let tasks = taskset1();
        let schedule = Schedule::schedule(&tasks);
        assert!(schedule.0[0].task.deadline < schedule.0[1].task.deadline);
        assert!(schedule.0[0].when < schedule.0[1].when);
    }

    fn taskset1() -> Vec<Task> {
        let task1 = Task {
            id: None,
            content: "do stuff".to_string(),
            deadline: UTC::now() + Duration::days(3),
            duration: Duration::hours(2),
            importance: 5,
        };
        let task2 = Task {
            id: None,
            content: "contemplate life".to_string(),
            deadline: UTC::now() + Duration::days(4),
            duration: Duration::hours(12),
            importance: 6,
        };
        vec![task1, task2]
    }
}
