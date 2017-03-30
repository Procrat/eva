#![feature(box_patterns)]

#[macro_use]
extern crate derive_new;
extern crate chrono;
extern crate take_mut;

#[cfg(test)]
#[macro_use]
extern crate assert_matches;

mod schedule_tree;

use chrono::{DateTime, Duration, UTC};
use schedule_tree::ScheduleTree;

#[derive(Debug, PartialEq, new, Clone)]
pub struct Task<'a> {
    content: &'a str,
    deadline: Option<DateTime<UTC>>,
    duration: Duration,
    importance: u8,
}

#[derive(Debug, new)]
struct ScheduledTask<'b, 'a: 'b> {
    task: &'b Task<'a>,
    when: DateTime<UTC>,
}

#[derive(Debug)]
pub struct Schedule<'b, 'a: 'b>(Vec<ScheduledTask<'b, 'a>>);

impl<'b, 'a> Schedule<'b, 'a> {
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
    pub fn schedule<'c: 'b>(tasks: &'c [Task<'a>]) -> Schedule<'b, 'a> {
        let mut tree = ScheduleTree::new();
        for task in tasks {
            // TODO unwrap
            let start = task.deadline.unwrap() - task.duration;
            // TODO schedule close before the deadline
            tree.schedule_exact(start, task.duration, task);
        }
        let importance_schedule = tree.iter().map(|entry| {
            ScheduledTask::new(entry.data, entry.start)
        }).collect();
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

    fn taskset1<'a>() -> Vec<Task<'a>> {
        let task1 = Task {
            content: "do stuff",
            deadline: Some(UTC::now() + Duration::days(3)),
            duration: Duration::hours(2),
            importance: 5,
        };
        let task2 = Task {
            content: "contemplate life",
            deadline: Some(UTC::now() + Duration::days(4)),
            duration: Duration::hours(12),
            importance: 6,
        };
        vec![task1, task2]
    }
}
