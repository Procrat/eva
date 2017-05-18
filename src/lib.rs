#![feature(box_patterns)]

#[macro_use]
extern crate derive_new;
extern crate chrono;
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_codegen;
extern crate take_mut;

#[cfg(test)]
#[macro_use]
extern crate assert_matches;

mod db;
mod schedule_tree;

use chrono::{DateTime, Duration, TimeZone, UTC};
use diesel::prelude::*;

use schedule_tree::ScheduleTree;


pub fn add(content: &str, deadline: &str, duration_hours: f64, importance: u32) {
    use db::tasks::dsl::tasks;

    let connection = db::make_connection();

    let deadline =
        UTC.datetime_from_str(deadline, "%-d %b %Y %-H:%M")
            .expect("Could not parse deadline. Please provide something like 4 Jul 6:05.");
    let duration = Duration::minutes((60.0 * duration_hours) as i64);
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

    diesel::delete(tasks.find(id as i32))
        .execute(&connection)
        .expect("Error removing task.");
}

pub fn print_schedule() {
    use db::tasks::dsl::tasks;

    let connection = db::make_connection();

    let tasks_ = tasks
        .load::<Task>(&connection)
        .expect("Error retrieving tasks.");
    for task in tasks_ {
        let prefix = match task.id {
            Some(id) => format!("{}.", id),
            None => "- ".to_string(),
        };
        println!("{} {}\n    (deadline: {}, duration: {}, importance: {})",
                 prefix,
                 task.content,
                 task.deadline,
                 task.duration,
                 task.importance);
    }
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


#[derive(Debug, new)]
struct ScheduledTask<'a> {
    task: &'a Task,
    when: DateTime<UTC>,
}

#[derive(Debug)]
pub struct Schedule<'a>(Vec<ScheduledTask<'a>>);

impl<'a> Schedule<'a> {
    /// Schedules tasks according to their deadlines, importance and duration.
    /// First, all tasks --- starting with the most important until the least important --- are
    /// scheduled as close as possible to their deadline.
    /// Next, all tasks --- starting with the most urgent one until the least urgent --- are put as
    /// close to the present as possible. This is too take care of contingencies like falling sick.
    /// A downside might be that a lot of time is spent doing urgent but non-important tasks.
    ///
    /// Args:
    ///     tasks: ordered list of tasks to schedule, ordered from most important to least
    ///     important.
    pub fn schedule<'b: 'a>(tasks: &'b [Task]) -> Schedule<'a> {
        let mut tree = ScheduleTree::new();
        for task in tasks {
            let start = task.deadline - task.duration;
            // TODO schedule close before the deadline
            tree.schedule_exact(start, task.duration, task);
        }
        let importance_schedule = tree.iter()
            .map(|entry| ScheduledTask::new(entry.data, entry.start))
            .collect();
        // TODO let urgent_first_schedule = importance_schedule
        Schedule(importance_schedule)
    }
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
