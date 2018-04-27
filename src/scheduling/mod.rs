use std::fmt;
use std::rc::Rc;

use chrono::prelude::*;
use chrono::Duration;
use itertools::Itertools;

use super::Task;
use self::schedule_tree::{Entry, ScheduleTree};

pub use self::errors::*;

mod schedule_tree;

#[allow(unused_doc_comment)]
mod errors {
    use ::Task;

    error_chain! {
        errors {
            DeadlineMissed(task: Task, already_missed: bool) {
                description("deadline missed")
                display("I could not schedule {} because you {} the deadline.\nYou might want to \
                        postpone this task or remove it if it's not longer relevant",
                        task,
                        if *already_missed { "missed" } else { "will miss" })
            }
            NotEnoughTime(task: Task) {
                description("not enough time")
                display("I could not schedule {} because you don't have enough time to do \
                        everything.\nYou might want to decide not to do some things or relax \
                        their deadlines",
                        task)
            }
            Internal(more_info: String) {
                description("internal error")
                display("An internal error occurred (This shouldn't happen.): {}", more_info)
            }
        }
    }
}

lazy_static! {
    static ref SCHEDULE_DELAY: Duration = Duration::minutes(1);
}


#[derive(Debug, new)]
pub struct ScheduledTask {
    pub task: Task,
    pub when: DateTime<Local>,
}

#[derive(Debug)]
pub struct Schedule(pub Vec<ScheduledTask>);

impl Schedule {
    /// Schedules tasks according to their deadlines, importance and duration.
    ///
    /// The exact algorithm might change in the future. Currently importance scheduling is being
    /// used. See `schedule_according_to_importance` for more details.
    ///
    /// Args:
    ///     tasks: iterable of tasks to schedule
    /// Returns an instance of Schedule which contains all tasks, each bound to certain date and
    /// time.
    #[allow(dead_code)]
    pub fn schedule<I>(tasks: I) -> Result<Schedule>
        where I: IntoIterator<Item=Task>
    {
        Schedule::schedule_according_to_importance(tasks)
    }

    /// Schedules `tasks` according to importance while making sure all deadlines are met.
    ///
    /// First, all tasks --- starting with the least important until the most important --- are
    /// scheduled as close as possible to their deadline. Next, all tasks --- starting with the
    /// most important until the least important --- are put as close to the present as possible.
    /// For ties on importance, more urgent tasks are scheduled later in the first phase and sooner
    /// in the second phase.
    ///
    /// This algorithm has a terrible performance at the moment and it doesn't work right when the
    /// lengths of the tasks aren't about the same, but it will do for now.
    pub fn schedule_according_to_importance<I>(tasks: I) -> Result<Schedule>
        where I: IntoIterator<Item=Task>
    {
        let mut tree: ScheduleTree<DateTime<Local>, Rc<Task>> = ScheduleTree::new();
        // Make sure things aren't scheduled before the algorithm is finished.
        let now = Local::now() + *SCHEDULE_DELAY;
        // Start by scheduling the least important tasks closest to the deadline, and so on.
        let mut tasks: Vec<Rc<Task>> = tasks.into_iter().map(Rc::new).collect::<Vec<_>>();
        tasks.sort_by_key(|task| (task.importance, now.signed_duration_since(task.deadline)));
        for task in &tasks {
            if task.deadline <= now + task.duration {
                bail!(ErrorKind::DeadlineMissed((**task).clone(), task.deadline <= now));
            }
            if ! tree.schedule_close_before(task.deadline, task.duration, Some(now), Rc::clone(task)) {
                bail!(ErrorKind::NotEnoughTime((**task).clone()));
            }
        }
        // Next, shift the most important tasks towards today, and so on, filling up the gaps.
        // Keep repeating that, until nothing changes anymore (i.e. all gaps are filled).
        let mut changed = !tree.is_empty();
        while changed {
            changed = false;
            for task in tasks.iter().rev() {
                let scheduled_entry = tree.unschedule(task)
                    .ok_or_else(|| ErrorKind::Internal(
                            "I couldn't unschedule a task".to_owned()))?;
                if ! tree.schedule_close_after(now, task.duration, Some(scheduled_entry.end),
                                               scheduled_entry.data) {
                    bail!(ErrorKind::Internal("I couldn't reschedule a task".to_owned()));
                }
                let new_start = tree.when_scheduled(task)
                    .ok_or_else(|| ErrorKind::Internal(
                            "I couldn't find a task that was just scheduled".to_owned()))?;
                if scheduled_entry.start != *new_start {
                    changed = true;
                    break;
                }
            }
        }
        Ok(Schedule::tree_to_schedule(tree))
    }

    /// Schedules `tasks` according to deadline first and then according to importance.
    ///
    /// First, all tasks --- starting with the least important until the most important --- are
    /// scheduled as close as possible to their deadline. Next, all tasks are put as close to the
    /// present as possible, keeping the order from the first scheduling phase.
    ///
    /// This algorithm is how Myrjam Van de Vijver does her personal scheduling. A benefit of doing
    /// it this way, is that it is highly robust against contingencies like falling sick. A
    /// disadvantage is that it gives more priority to urgent but less important tasks than to
    /// important but less urgent tasks.
    pub fn schedule_according_to_myrjam<I>(tasks: I) -> Result<Schedule>
        where I: IntoIterator<Item=Task>
    {
        let mut tree: ScheduleTree<DateTime<Local>, Rc<Task>> = ScheduleTree::new();
        // Make sure things aren't scheduled before the algorithm is finished.
        let now = Local::now() + *SCHEDULE_DELAY;
        // Start by scheduling the least important tasks closest to the deadline, and so on.
        let mut tasks: Vec<Rc<Task>> = tasks.into_iter().map(Rc::new).collect::<Vec<_>>();
        tasks.sort_by_key(|task| task.importance);
        for task in tasks {
            if task.deadline <= now + task.duration {
                bail!(ErrorKind::DeadlineMissed((*task).clone(), task.deadline <= now));
            }
            if ! tree.schedule_close_before(task.deadline, task.duration, Some(now), Rc::clone(&task)) {
                bail!(ErrorKind::NotEnoughTime((*task).clone()));
            }
        }
        // Next, shift the all tasks towards the present, filling up the gaps.
        let mut entries = vec![];
        for entry in tree.iter() {
            entries.push(Entry {
                start: entry.start,
                end: entry.end,
                data: Rc::clone(entry.data),
            });
        }
        for entry in entries {
            let scheduled_entry = tree.unschedule(&entry.data)
                .ok_or_else(|| ErrorKind::Internal("I couldn't unschedule a task".to_owned()))?;
            let task = scheduled_entry.data;
            if ! tree.schedule_close_after(now, task.duration, Some(scheduled_entry.end), task) {
                bail!(ErrorKind::Internal("I couldn't reschedule a task".to_owned()));
            }
        }
        Ok(Schedule::tree_to_schedule(tree))
    }

    fn tree_to_schedule(tree: ScheduleTree<DateTime<Local>, Rc<Task>>) -> Schedule {
        let scheduled_tasks = tree.into_iter()
            .map(|entry| ScheduledTask::new((*entry.data).clone(), entry.start))
            .collect();
        Schedule(scheduled_tasks)
    }
}

impl fmt::Display for Schedule {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "Schedule:\n  "));
        write!(f, "{}", self.0.iter().join("\n  "))
    }
}

impl fmt::Display for ScheduledTask {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {}",
               format_datetime(self.when),
               self.task)
    }
}

impl fmt::Display for Task {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let prefix = format!("{}. ", self.id);
        write!(f, "{}{}\n    (deadline: {}, duration: {}, importance: {})",
               prefix,
               self.content,
               format_datetime(self.deadline),
               format_duration(self.duration),
               self.importance)
    }
}

fn format_datetime(datetime: DateTime<Local>) -> String {
    let format = if datetime.year() == Local::now().year() {
        "%a %-d %b %-H:%M"
    } else {
        "%a %-d %b %Y %-H:%M"
    };
    datetime.format(format).to_string()
}

fn format_duration(duration: Duration) -> String {
    if duration.num_minutes() > 0 {
        format!("{}h{}", duration.num_hours(), duration.num_minutes() % 60)
    } else {
        format!("{}h", duration.num_hours())
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! test_generic_properties {
        ($($algorithm_name:ident: $schedule_fn:expr,)*) => {
            $(
                mod $algorithm_name {
                    use super::*;

                    #[test]
                    fn all_tasks_are_scheduled() {
                        for tasks in vec![taskset_of_myrjam(), taskset_just_in_time()] {
                            let schedule = $schedule_fn(tasks.clone()).unwrap();
                            assert_eq!(tasks.len(), schedule.0.len());
                            for scheduled_task in schedule.0.iter() {
                                assert!(tasks.contains(&scheduled_task.task));
                            }
                            for task in tasks {
                                assert!(schedule.0.iter().any(|scheduled_task| scheduled_task.task == task));
                            }
                        }
                    }

                    #[test]
                    fn tasks_are_in_scheduled_in_time() {
                        for tasks in vec![taskset_of_myrjam(), taskset_just_in_time()] {
                            let schedule = $schedule_fn(tasks.clone()).unwrap();
                            for scheduled_task in schedule.0.iter() {
                                assert!(scheduled_task.when <= scheduled_task.task.deadline);
                            }
                        }
                    }

                    #[test]
                    fn schedule_just_in_time() {
                        let tasks = taskset_just_in_time();
                        let schedule = $schedule_fn(tasks.clone()).unwrap();
                        assert_eq!(schedule.0[0].task, tasks[0]);
                        assert_eq!(schedule.0[1].task, tasks[1]);
                        assert!(are_approx_equal(schedule.0[0].when,
                                                 Local::now() + *SCHEDULE_DELAY));
                        assert!(are_approx_equal(schedule.0[1].when,
                                                 Local::now() - *SCHEDULE_DELAY
                                                 + Duration::days(23 * 365)));
                    }

                    #[test]
                    fn schedule_sets_of_two() {
                        let mut tasks = vec![Task {
                            id: 0,
                            content: "find meaning to life".to_string(),
                            deadline: Local::now() + Duration::hours(1),
                            duration: Duration::hours(1) - *SCHEDULE_DELAY * 2,
                            importance: 6,
                        },
                        Task {
                            id: 1,
                            content: "stop giving a fuck".to_string(),
                            deadline: Local::now() + Duration::hours(3),
                            duration: Duration::hours(2) - *SCHEDULE_DELAY * 2,
                            importance: 5,
                        }];
                        // Normal scheduling
                        {
                            let schedule = $schedule_fn(tasks.clone()).unwrap();
                            assert_eq!(schedule.0[0].task, tasks[0]);
                            assert_eq!(schedule.0[1].task, tasks[1]);
                        }

                        // Reversing the importance should maintain the scheduled order, because it's the only way
                        // to meet the deadlines.
                        tasks[0].importance = 5;
                        tasks[1].importance = 6;
                        {
                            let schedule = $schedule_fn(tasks.clone()).unwrap();
                            assert_eq!(schedule.0[0].task, tasks[0]);
                            assert_eq!(schedule.0[1].task, tasks[1]);
                        }

                        // Leveling the deadlines should make the more important task be scheduled first again.
                        tasks[0].deadline = Local::now() + Duration::hours(3);
                        let schedule = $schedule_fn(tasks.clone()).unwrap();
                        assert_eq!(schedule.0[0].task, tasks[1]);
                        assert_eq!(schedule.0[1].task, tasks[0]);
                    }

                    #[test]
                    fn no_schedule() {
                        let tasks = vec![];
                        let schedule = $schedule_fn(tasks).unwrap();
                        assert!(schedule.0.is_empty());
                    }

                    #[test]
                    fn missed_deadline() {
                        let tasks = taskset_with_missed_deadline();
                        assert_matches!($schedule_fn(tasks),
                                        Err(Error(ErrorKind::DeadlineMissed(_, true), _)));
                    }

                    #[test]
                    fn impossible_deadline() {
                        let tasks = taskset_with_impossible_deadline();
                        assert_matches!($schedule_fn(tasks),
                                        Err(Error(ErrorKind::DeadlineMissed(_, false), _)));
                    }

                    #[test]
                    fn out_of_time() {
                        let tasks = taskset_impossible_combination();
                        assert_matches!($schedule_fn(tasks),
                                        Err(Error(ErrorKind::NotEnoughTime(_), _)));
                    }
                }
             )*
        }
    }

    test_generic_properties! {
        importance: Schedule::schedule_according_to_importance,
        urgency: Schedule::schedule_according_to_myrjam,
        default: Schedule::schedule,
    }

    // Note that some of these task sets are not representative at all, since tasks should be small
    // and actionable. Things like taking over the world should be handled by Eva in a higher
    // abstraction level in something like projects, which should not be scheduled.

    fn taskset_of_myrjam() -> Vec<Task> {
        let task1 = Task {
            id: 1,
            content: "take over the world".to_string(),
            deadline: Local::now() + Duration::days(6 * 365),
            duration: Duration::hours(1000),
            importance: 10,
        };
        let task2 = Task {
            id: 2,
            content: "make onion soup".to_string(),
            deadline: Local::now() + Duration::hours(2),
            duration: Duration::hours(1),
            importance: 3,
        };
        let task3 = Task {
            id: 3,
            content: "publish Commander Mango 3".to_string(),
            deadline: Local::now() + Duration::days(365 / 2),
            duration: Duration::hours(50),
            importance: 6,
        };
        let task4 = Task {
            id: 4,
            content: "sculpt".to_string(),
            deadline: Local::now() + Duration::days(30),
            duration: Duration::hours(10),
            importance: 4,
        };
        let task5 = Task {
            id: 5,
            content: "organise birthday present".to_string(),
            deadline: Local::now() + Duration::days(30),
            duration: Duration::hours(5),
            importance: 10,
        };
        let task6 = Task {
            id: 6,
            content: "make dentist appointment".to_string(),
            deadline: Local::now() + Duration::days(7),
            duration: Duration::minutes(10),
            importance: 5,
        };
        vec![task1, task2, task3, task4, task5, task6]
    }

    fn taskset_just_in_time() -> Vec<Task> {
        let task1 = Task {
            id: 1,
            content: "go to school".to_string(),
            deadline: Local::now() + Duration::days(23 * 365),
            duration: Duration::days(23 * 365) - *SCHEDULE_DELAY * 2,
            importance: 5,
        };
        let task2 = Task {
            id: 2,
            content: "work till you die".to_string(),
            deadline: Local::now() + Duration::days(65 * 365),
            duration: Duration::days(42 * 365),
            importance: 6,
        };
        vec![task1, task2]
    }

    #[test]
    fn schedule_for_myrjam() {
        let tasks = taskset_of_myrjam();
        let schedule = Schedule::schedule_according_to_myrjam(tasks.clone()).unwrap();
        let mut expected_when = Local::now() + *SCHEDULE_DELAY;
        // 1. Make onion soup, 1h, 3, in 2 hours
        assert_eq!(schedule.0[0].task, tasks[1]);
        assert!(are_approx_equal(schedule.0[0].when, expected_when));
        expected_when = expected_when + Duration::hours(1);
        // 5. Make dentist appointment, 10m, 5, in 7 days
        assert_eq!(schedule.0[1].task, tasks[5]);
        assert!(are_approx_equal(schedule.0[1].when, expected_when));
        expected_when = expected_when + Duration::minutes(10);
        // 4. Organise birthday present, 5h, 10, in 30 days
        assert_eq!(schedule.0[2].task, tasks[4]);
        assert!(are_approx_equal(schedule.0[2].when, expected_when));
        expected_when = expected_when + Duration::hours(5);
        // 3. Sculpt, 10h, 4, in 30 days
        assert_eq!(schedule.0[3].task, tasks[3]);
        assert!(are_approx_equal(schedule.0[3].when, expected_when));
        expected_when = expected_when + Duration::hours(10);
        // 2. Public Commander Mango 3, 50h, 6, in 6 months
        assert_eq!(schedule.0[4].task, tasks[2]);
        assert!(are_approx_equal(schedule.0[4].when, expected_when));
        expected_when = expected_when + Duration::hours(50);
        // 0. Take over world, 1000h, 10, in 10 years
        assert_eq!(schedule.0[5].task, tasks[0]);
        assert!(are_approx_equal(schedule.0[5].when, expected_when));
    }

    #[test]
    fn schedule_myrjams_schedule_by_importance() {
        let tasks = taskset_of_myrjam();
        let schedule = Schedule::schedule_according_to_importance(tasks.clone()).unwrap();
        let mut expected_when = Local::now() + *SCHEDULE_DELAY;
        // 5. Make dentist appointment, 10m, 5, in 7 days
        assert_eq!(schedule.0[0].task, tasks[5]);
        assert!(are_approx_equal(schedule.0[0].when, expected_when));
        expected_when = expected_when + Duration::minutes(10);
        // 1. Make onion soup, 1h, 3, in 2 hours
        assert_eq!(schedule.0[1].task, tasks[1]);
        assert!(are_approx_equal(schedule.0[1].when, expected_when));
        expected_when = expected_when + Duration::hours(1);
        // 4. Organise birthday present, 5h, 10, in 30 days
        assert_eq!(schedule.0[2].task, tasks[4]);
        assert!(are_approx_equal(schedule.0[2].when, expected_when));
        expected_when = expected_when + Duration::hours(5);
        // 2. Public Commander Mango 3, 50h, 6, in 6 months
        assert_eq!(schedule.0[3].task, tasks[2]);
        assert!(are_approx_equal(schedule.0[3].when, expected_when));
        expected_when = expected_when + Duration::hours(50);
        // 3. Sculpt, 10h, 4, in 30 days
        assert_eq!(schedule.0[4].task, tasks[3]);
        assert!(are_approx_equal(schedule.0[4].when, expected_when));
        expected_when = expected_when + Duration::hours(10);
        // 0. Take over world, 1000h, 10, in 10 years
        assert_eq!(schedule.0[5].task, tasks[0]);
        assert!(are_approx_equal(schedule.0[5].when, expected_when));
    }

    fn taskset_of_gandalf() -> Vec<Task> {
        vec![
            Task {
                id: 0,
                content: "Think of plan to get rid of The Ring".to_string(),
                deadline: Local::now() + Duration::days(12) + Duration::hours(15),
                duration: Duration::days(2),
                importance: 9
            },
            Task {
                id: 1,
                content: "Ask advice from Saruman".to_string(),
                deadline: Local::now() + Duration::days(8) + Duration::hours(15),
                duration: Duration::days(3),
                importance: 4
            },
            Task {
                id: 2,
                content: "Visit Bilbo in Rivendel".to_string(),
                deadline: Local::now() + Duration::days(13) + Duration::hours(15),
                duration: Duration::days(2),
                importance: 2
            },
            Task {
                id: 3,
                content: "Make some firework for the hobbits".to_string(),
                deadline: Local::now() + Duration::hours(33),
                duration: Duration::hours(3),
                importance: 3
            },
            Task {
                id: 4,
                content: "Get riders of Rohan to help Gondor".to_string(),
                deadline: Local::now() + Duration::days(21) + Duration::hours(15),
                duration: Duration::days(7),
                importance: 7,
            },
            Task {
                id: 5,
                content: "Find some good pipe-weed".to_string(),
                deadline: Local::now() + Duration::days(2) + Duration::hours(15),
                duration: Duration::hours(1),
                importance: 8
            },
            Task {
                id: 6,
                content: "Go shop for white clothing".to_string(),
                deadline: Local::now() + Duration::days(33) + Duration::hours(15),
                duration: Duration::hours(2),
                importance: 3
            },
            Task {
                id: 7,
                content: "Prepare epic-sounding one-liners".to_string(),
                deadline: Local::now() + Duration::hours(34),
                duration: Duration::hours(2),
                importance: 10
            },
            Task {
                id: 8,
                content: "Recharge staff batteries".to_string(),
                deadline: Local::now() + Duration::days(1) + Duration::hours(15),
                duration: Duration::minutes(30),
                importance: 5
            },
        ]
    }

    #[test]
    fn schedule_gandalfs_schedule_by_importance() {
        let tasks = taskset_of_gandalf();
        let schedule = Schedule::schedule_according_to_importance(tasks.clone()).unwrap();
        let mut expected_when = Local::now() + *SCHEDULE_DELAY;
        // 7. Prepare epic-sounding one-liners
        assert_eq!(schedule.0[0].task, tasks[7]);
        assert!(are_approx_equal(schedule.0[0].when, expected_when));
        expected_when = expected_when + Duration::hours(2);
        // 5. Find some good pipe-weed
        assert_eq!(schedule.0[1].task, tasks[5]);
        assert!(are_approx_equal(schedule.0[1].when, expected_when));
        expected_when = expected_when + Duration::hours(1);
        // 8. Recharge staff batteries
        assert_eq!(schedule.0[2].task, tasks[8]);
        assert!(are_approx_equal(schedule.0[2].when, expected_when));
        expected_when = expected_when + Duration::minutes(30);
        // 3. Make some firework for the hobbits
        assert_eq!(schedule.0[3].task, tasks[3]);
        assert!(are_approx_equal(schedule.0[3].when, expected_when));
        expected_when = expected_when + Duration::hours(3);
        // 0. Think of plan to get rid of The Ring
        assert_eq!(schedule.0[4].task, tasks[0]);
        assert!(are_approx_equal(schedule.0[4].when, expected_when));
        expected_when = expected_when + Duration::days(2);
        // 1. Ask advice from Saruman
        assert_eq!(schedule.0[5].task, tasks[1]);
        assert!(are_approx_equal(schedule.0[5].when, expected_when));
        expected_when = expected_when + Duration::days(3);
        // 6. Go shop for white clothing
        assert_eq!(schedule.0[6].task, tasks[6]);
        assert!(are_approx_equal(schedule.0[6].when, expected_when));
        expected_when = expected_when + Duration::hours(2);
        // 2. Visit Bilbo in Rivendel
        assert_eq!(schedule.0[7].task, tasks[2]);
        assert!(are_approx_equal(schedule.0[7].when, expected_when));
        expected_when = expected_when + Duration::days(2);
        // 4. Get riders of Rohan to help Gondor
        assert_eq!(schedule.0[8].task, tasks[4]);
        assert!(are_approx_equal(schedule.0[8].when, expected_when));
    }

    fn taskset_with_missed_deadline() -> Vec<Task> {
        let task1 = Task {
            id: 1,
            content: "conquer the world".to_string(),
            deadline: Local::now() + Duration::days(3),
            duration: Duration::days(1),
            importance: 5,
        };
        let task2 = Task {
            id: 2,
            content: "save the world".to_string(),
            deadline: Local::now() - Duration::days(1),
            duration: Duration::minutes(5),
            importance: 5,
        };
        vec![task1, task2]
    }

    fn taskset_with_impossible_deadline() -> Vec<Task> {
        let task1 = Task {
            id: 1,
            content: "conquer the world".to_string(),
            deadline: Local::now() + Duration::days(3),
            duration: Duration::days(1),
            importance: 5,
        };
        let task2 = Task {
            id: 2,
            content: "save the world".to_string(),
            deadline: Local::now() + Duration::hours(23),
            duration: Duration::days(1),
            importance: 5,
        };
        vec![task1, task2]
    }

    fn taskset_impossible_combination() -> Vec<Task> {
        let task1 = Task {
            id: 1,
            content: "Learn Rust".to_string(),
            deadline: Local::now() + Duration::days(1),
            duration: Duration::days(1) - *SCHEDULE_DELAY * 2,
            importance: 5,
        };
        let task2 = Task {
            id: 2,
            content: "Program Eva".to_string(),
            deadline: Local::now() + Duration::days(2),
            duration: Duration::days(1) + Duration::minutes(1),
            importance: 5,
        };
        vec![task1, task2]
    }

    fn are_approx_equal(datetime1: DateTime<Local>, datetime2: DateTime<Local>) -> bool {
        datetime1 < datetime2 + Duration::seconds(2)
            && datetime2 < datetime1 + Duration::seconds(2)
    }
}
