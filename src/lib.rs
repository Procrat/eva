#![feature(box_patterns)]

extern crate chrono;
#[macro_use]
extern crate derive_new;
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_codegen;
#[macro_use]
extern crate error_chain;
extern crate itertools;
#[macro_use]
extern crate lazy_static;
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

use configuration::Configuration;
use schedule_tree::ScheduleTree;

pub use errors::{Error, ErrorKind, Result, ResultExt};

#[macro_use]
mod util;

pub mod configuration;
mod db;
mod schedule_tree;

#[allow(unused_doc_comment)]
mod errors {
    error_chain! {
        errors {
            Parse(what: String, how_it_should_be: String) {
                description("parse error")
                display("I could not parse the {}. {}", what, how_it_should_be)
            }
            Schedule(why: String) {
                description("scheduling error")
                display("I could not schedule your tasks {}", why)
            }
            Database(when: String) {
                description("database error")
                display("A database error occurred {}", when)
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


pub fn add(configuration: &Configuration,
           content: &str,
           deadline: &str,
           duration: &str,
           importance: u32)
    -> Result<()>
{
    use db::tasks::dsl::tasks;

    let connection = db::make_connection(configuration)?;

    let deadline = parse_datetime(deadline)?;
    let duration = parse_duration(duration)?;
    let new_task = Task {
        id: None,
        content: content.to_string(),
        deadline,
        duration,
        importance,
    };

    diesel::insert(&new_task)
        .into(tasks)
        .execute(&connection)
        .chain_err(|| ErrorKind::Database("while trying to add a task".to_owned()))?;

    Ok(())
}

pub fn remove(configuration: &Configuration, id: u32) -> Result<()> {
    use db::tasks::dsl::tasks;

    let connection = db::make_connection(configuration)?;

    let amount_deleted =
        diesel::delete(tasks.find(id as i32))
        .execute(&connection)
        .chain_err(|| ErrorKind::Database("while trying to remove a task".to_owned()))?;

    if amount_deleted == 0 {
        bail!(ErrorKind::Database(format!("while trying to find the task with id {}", id)));
    } else if amount_deleted > 1 {
        bail!(ErrorKind::Internal("multiple tasks got deleted".to_owned()));
    }

    Ok(())
}

pub fn set(configuration: &Configuration, field_name: &str, id: u32, value: &str) -> Result<()> {
    assert!(["content", "deadline", "duration", "importance"].contains(&field_name));

    use db::tasks::dsl::tasks;

    let connection = db::make_connection(configuration)?;

    let mut task: Task = tasks.find(id as i32)
        .first(&connection)
        .chain_err(|| ErrorKind::Database("while trying to retrieve a task".to_owned()))?;

    match field_name {
        "content" => task.content = value.to_string(),
        "deadline" => task.deadline = parse_datetime(value)?,
        "duration" => task.duration = parse_duration(value)?,
        "importance" => task.importance = parse_importance(value)?,
        _ => unreachable!(),
    }

    let amount_updated = diesel::update(&task)
        .set(task)
        .execute(&connection)
        .chain_err(|| ErrorKind::Database("while trying to update a task".to_owned()))?;

    if amount_updated == 0 {
        bail!(ErrorKind::Database("while trying to update a task".to_owned()));
    } else if amount_updated > 1 {
        bail!(ErrorKind::Internal("multiple tasks got deleted".to_owned()));
    }

    Ok(())
}

pub fn print_schedule(configuration: &Configuration, strategy: &str) -> Result<()> {
    assert!(["importance", "urgency"].contains(&strategy));

    use db::tasks::dsl::tasks;

    let connection = db::make_connection(configuration)?;

    let tasks_ = tasks
        .load::<Task>(&connection)
        .chain_err(|| ErrorKind::Database("while trying to retrieve tasks".to_owned()))?;

    println!("Tasks:");
    for task in &tasks_ {
        println!("  {}", task);
    }
    println!();

    let schedule = match strategy {
        "importance" => Schedule::schedule_according_to_importance(&tasks_),
        "urgency" => Schedule::schedule_according_to_myrjam(&tasks_),
        _ => unreachable!(),
    }?;
    println!("{}", schedule);

    Ok(())
}


#[derive(Debug, Eq, new, Clone)]
pub struct Task {
    id: Option<u32>,
    content: String,
    deadline: DateTime<Local>,
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
        self.duration.to_std()
            .expect(&format!("Internal error: duration of {} was negative", self))
            .hash(state);
        self.importance.hash(state);
    }
}


#[derive(Debug, new)]
struct ScheduledTask<'a> {
    task: &'a Task,
    when: DateTime<Local>,
}

#[derive(Debug)]
pub struct Schedule<'a>(Vec<ScheduledTask<'a>>);

impl<'a> Schedule<'a> {
    /// Schedules tasks according to their deadlines, importance and duration.
    ///
    /// The exact algorithm might change in the future. Currently importance scheduling is being
    /// used. See `schedule_according_to_importance` for more details.
    ///
    /// Args:
    ///     tasks: iterable of tasks to schedule
    /// Returns an instance of Schedule which contains all tasks, each bound to certain date and
    /// time.
    pub fn schedule<'b: 'a, I>(tasks: I) -> Result<Schedule<'a>>
        where I: IntoIterator<Item=&'b Task>
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
    fn schedule_according_to_importance<'b: 'a, I>(tasks: I) -> Result<Schedule<'a>>
        where I: IntoIterator<Item=&'b Task>
    {
        let mut tree = ScheduleTree::new();
        // Make sure things aren't scheduled before the algorithm is finished.
        let now = Local::now() + *SCHEDULE_DELAY;
        // Start by scheduling the least important tasks closest to the deadline, and so on.
        let mut tasks: Vec<&Task> = tasks.into_iter().collect::<Vec<_>>();
        tasks.sort_by_key(|task| (task.importance, now.signed_duration_since(task.deadline)));
        for task in &tasks {
            if task.deadline <= now + task.duration {
                bail!(ErrorKind::Schedule(format!(
                    "because you {} the deadline of {}.\nYou might want to postpone this task \
                    or remove it if it's no longer relevant",
                    if task.deadline <= now { "missed" } else { "will miss" },
                    task)));
            }
            if ! tree.schedule_close_before(task.deadline, task.duration, Some(now), *task) {
                bail!(ErrorKind::Schedule(
                    "because you don't have enought time to do everything.\nYou might want to \
                    decide not to do some things or relax their deadlines".to_owned()));
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
                if ! tree.schedule_close_after(now, task.duration, Some(scheduled_entry.end), *task) {
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
        Ok(Schedule::tree_to_schedule(&tree))
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
    fn schedule_according_to_myrjam<'b: 'a, I>(tasks: I) -> Result<Schedule<'a>>
        where I: IntoIterator<Item=&'b Task>
    {
        let mut tree = ScheduleTree::new();
        // Make sure things aren't scheduled before the algorithm is finished.
        let now = Local::now() + *SCHEDULE_DELAY;
        // Start by scheduling the least important tasks closest to the deadline, and so on.
        let mut tasks: Vec<&Task> = tasks.into_iter().collect::<Vec<_>>();
        tasks.sort_by_key(|task| task.importance);
        for task in &tasks {
            if task.deadline <= now + task.duration {
                bail!(ErrorKind::Schedule(format!(
                    "because you {} the deadline of {}.\nYou might want to postpone this task \
                    or remove it if it's no longer relevant",
                    if task.deadline <= now { "missed" } else { "will miss" },
                    task)));
            }
            eprintln!("Calling schedule_close_before w/ {} now={}", task, now);
            if ! tree.schedule_close_before(task.deadline, task.duration, Some(now), *task) {
                bail!(ErrorKind::Schedule(
                    "because you don't have enought time to do everything.\nYou might want to \
                    decide not to do some things or relax their deadlines".to_owned()));
            }
        }
        // Next, shift the all tasks towards the present, filling up the gaps.
        for entry in tree.iter().collect::<Vec<_>>() {
            let task = entry.data;
            let scheduled_entry = tree.unschedule(task)
                .ok_or_else(|| ErrorKind::Internal("I couldn't unschedule a task".to_owned()))?;
            if ! tree.schedule_close_after(now, task.duration, Some(scheduled_entry.end), task) {
                bail!(ErrorKind::Internal("I couldn't reschedule a task".to_owned()));
            }
        }
        Ok(Schedule::tree_to_schedule(&tree))
    }

    fn tree_to_schedule(tree: &ScheduleTree<'a, DateTime<Local>, Task>) -> Schedule<'a> {
        let scheduled_tasks = tree.iter()
            .map(|entry| ScheduledTask::new(entry.data, entry.start))
            .collect();
        Schedule(scheduled_tasks)
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
        let prefix = self.id.map_or("".to_string(), |id| format!("{}. ", id));
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

fn parse_datetime(datetime: &str) -> Result<DateTime<Local>> {
    Local.datetime_from_str(datetime, "%-d %b %Y %-H:%M")
        .chain_err(|| {
            ErrorKind::Parse("deadline".to_owned(),
                             "Please provide something like '4 Jul 2017 6:05'".to_owned())
        })
}

fn parse_importance(importance_str: &str) -> Result<u32> {
    importance_str.parse()
        .chain_err(|| ErrorKind::Parse("importance".to_owned(),
                                       "Please supply a valid integer".to_owned()))
}

fn parse_duration(duration_hours: &str) -> Result<Duration> {
    let hours: f64 = duration_hours.parse()
        .chain_err(|| ErrorKind::Parse("duration".to_owned(),
                                       "Please supply a valid, real number".to_owned()))?;
    if hours <= 0.0 {
        bail!(ErrorKind::Parse("duration".to_owned(),
                               "Please supply a positive number".to_owned()));
    }
    Ok(Duration::minutes((60.0 * hours) as i64))
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
                        for tasks in [taskset_of_myrjam(), taskset_just_in_time()].iter() {
                            let schedule = $schedule_fn(tasks).unwrap();
                            assert_eq!(tasks.len(), schedule.0.len());
                            for scheduled_task in schedule.0.iter() {
                                assert!(tasks.contains(scheduled_task.task));
                            }
                            for task in tasks {
                                assert!(schedule.0.iter().any(|scheduled_task| scheduled_task.task == task));
                            }
                        }
                    }

                    #[test]
                    fn tasks_are_in_scheduled_in_time() {
                        for tasks in [taskset_of_myrjam(), taskset_just_in_time()].iter() {
                            let schedule = $schedule_fn(tasks).unwrap();
                            for scheduled_task in schedule.0.iter() {
                                assert!(scheduled_task.when <= scheduled_task.task.deadline);
                            }
                        }
                    }

                    #[test]
                    fn schedule_just_in_time() {
                        let tasks = taskset_just_in_time();
                        let schedule = $schedule_fn(&tasks).unwrap();
                        assert_eq!(*schedule.0[0].task, tasks[0]);
                        assert_eq!(*schedule.0[1].task, tasks[1]);
                        assert!(are_approx_equal(schedule.0[0].when,
                                                 Local::now() + *SCHEDULE_DELAY));
                        assert!(are_approx_equal(schedule.0[1].when,
                                                 Local::now() - *SCHEDULE_DELAY
                                                 + Duration::days(23 * 365)));
                    }

                    #[test]
                    fn schedule_sets_of_two() {
                        let mut tasks = vec![Task {
                            id: None,
                            content: "find meaning to life".to_string(),
                            deadline: Local::now() + Duration::hours(1),
                            duration: Duration::hours(1) - *SCHEDULE_DELAY * 2,
                            importance: 6,
                        },
                        Task {
                            id: None,
                            content: "stop giving a fuck".to_string(),
                            deadline: Local::now() + Duration::hours(3),
                            duration: Duration::hours(2) - *SCHEDULE_DELAY * 2,
                            importance: 5,
                        }];
                        // Normal scheduling
                        {
                            let schedule = $schedule_fn(&tasks).unwrap();
                            assert_eq!(*schedule.0[0].task, tasks[0]);
                            assert_eq!(*schedule.0[1].task, tasks[1]);
                        }

                        // Reversing the importance should maintain the scheduled order, because it's the only way
                        // to meet the deadlines.
                        tasks[0].importance = 5;
                        tasks[1].importance = 6;
                        {
                            let schedule = $schedule_fn(&tasks).unwrap();
                            assert_eq!(*schedule.0[0].task, tasks[0]);
                            assert_eq!(*schedule.0[1].task, tasks[1]);
                        }

                        // Leveling the deadlines should make the more important task be scheduled first again.
                        tasks[0].deadline = Local::now() + Duration::hours(3);
                        let schedule = $schedule_fn(&tasks).unwrap();
                        assert_eq!(*schedule.0[0].task, tasks[1]);
                        assert_eq!(*schedule.0[1].task, tasks[0]);
                    }

                    #[test]
                    fn no_schedule() {
                        let tasks = [];
                        let schedule = $schedule_fn(&tasks).unwrap();
                        assert!(schedule.0.is_empty());
                    }

                    #[test]
                    fn missed_deadline() {
                        let tasks = taskset_with_missed_deadline();
                        assert_matches!($schedule_fn(&tasks),
                                        Err(Error(ErrorKind::Schedule(_), _)));
                    }

                    #[test]
                    fn out_of_time() {
                        let tasks = taskset_impossible();
                        assert_matches!($schedule_fn(&tasks),
                                        Err(Error(ErrorKind::Schedule(_), _)));
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
            id: None,
            content: "take over the world".to_string(),
            deadline: Local::now() + Duration::days(6 * 365),
            duration: Duration::hours(1000),
            importance: 10,
        };
        let task2 = Task {
            id: None,
            content: "make onion soup".to_string(),
            deadline: Local::now() + Duration::hours(2),
            duration: Duration::hours(1),
            importance: 3,
        };
        let task3 = Task {
            id: None,
            content: "publish Commander Mango 3".to_string(),
            deadline: Local::now() + Duration::days(365 / 2),
            duration: Duration::hours(50),
            importance: 6,
        };
        let task4 = Task {
            id: None,
            content: "sculpt".to_string(),
            deadline: Local::now() + Duration::days(30),
            duration: Duration::hours(10),
            importance: 4,
        };
        let task5 = Task {
            id: None,
            content: "organise birthday present".to_string(),
            deadline: Local::now() + Duration::days(30),
            duration: Duration::hours(5),
            importance: 10,
        };
        let task6 = Task {
            id: None,
            content: "make dentist appointment".to_string(),
            deadline: Local::now() + Duration::days(7),
            duration: Duration::minutes(10),
            importance: 5,
        };
        vec![task1, task2, task3, task4, task5, task6]
    }

    fn taskset_just_in_time() -> Vec<Task> {
        let task1 = Task {
            id: None,
            content: "go to school".to_string(),
            deadline: Local::now() + Duration::days(23 * 365),
            duration: Duration::days(23 * 365) - *SCHEDULE_DELAY * 2,
            importance: 5,
        };
        let task2 = Task {
            id: None,
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
        let schedule = Schedule::schedule_according_to_myrjam(&tasks).unwrap();
        let mut expected_when = Local::now() + *SCHEDULE_DELAY;
        // 1. Make onion soup, 1h, 3, in 2 hours
        assert_eq!(*schedule.0[0].task, tasks[1]);
        assert!(are_approx_equal(schedule.0[0].when, expected_when));
        expected_when = expected_when + Duration::hours(1);
        // 5. Make dentist appointment, 10m, 5, in 7 days
        assert_eq!(*schedule.0[1].task, tasks[5]);
        assert!(are_approx_equal(schedule.0[1].when, expected_when));
        expected_when = expected_when + Duration::minutes(10);
        // 4. Organise birthday present, 5h, 10, in 30 days
        assert_eq!(*schedule.0[2].task, tasks[4]);
        assert!(are_approx_equal(schedule.0[2].when, expected_when));
        expected_when = expected_when + Duration::hours(5);
        // 3. Sculpt, 10h, 4, in 30 days
        assert_eq!(*schedule.0[3].task, tasks[3]);
        assert!(are_approx_equal(schedule.0[3].when, expected_when));
        expected_when = expected_when + Duration::hours(10);
        // 2. Public Commander Mango 3, 50h, 6, in 6 months
        assert_eq!(*schedule.0[4].task, tasks[2]);
        assert!(are_approx_equal(schedule.0[4].when, expected_when));
        expected_when = expected_when + Duration::hours(50);
        // 0. Take over world, 1000h, 10, in 10 years
        assert_eq!(*schedule.0[5].task, tasks[0]);
        assert!(are_approx_equal(schedule.0[5].when, expected_when));
    }

    #[test]
    fn schedule_myrjams_schedule_by_importance() {
        let tasks = taskset_of_myrjam();
        let schedule = Schedule::schedule_according_to_importance(&tasks).unwrap();
        let mut expected_when = Local::now() + *SCHEDULE_DELAY;
        // 5. Make dentist appointment, 10m, 5, in 7 days
        assert_eq!(*schedule.0[0].task, tasks[5]);
        assert!(are_approx_equal(schedule.0[0].when, expected_when));
        expected_when = expected_when + Duration::minutes(10);
        // 1. Make onion soup, 1h, 3, in 2 hours
        assert_eq!(*schedule.0[1].task, tasks[1]);
        assert!(are_approx_equal(schedule.0[1].when, expected_when));
        expected_when = expected_when + Duration::hours(1);
        // 4. Organise birthday present, 5h, 10, in 30 days
        assert_eq!(*schedule.0[2].task, tasks[4]);
        assert!(are_approx_equal(schedule.0[2].when, expected_when));
        expected_when = expected_when + Duration::hours(5);
        // 2. Public Commander Mango 3, 50h, 6, in 6 months
        assert_eq!(*schedule.0[3].task, tasks[2]);
        assert!(are_approx_equal(schedule.0[3].when, expected_when));
        expected_when = expected_when + Duration::hours(50);
        // 3. Sculpt, 10h, 4, in 30 days
        assert_eq!(*schedule.0[4].task, tasks[3]);
        assert!(are_approx_equal(schedule.0[4].when, expected_when));
        expected_when = expected_when + Duration::hours(10);
        // 0. Take over world, 1000h, 10, in 10 years
        assert_eq!(*schedule.0[5].task, tasks[0]);
        assert!(are_approx_equal(schedule.0[5].when, expected_when));
    }

    fn taskset_of_gandalf() -> Vec<Task> {
        vec![
            Task {
                id: None,
                content: "Think of plan to get rid of The Ring".to_string(),
                deadline: Local::now() + Duration::days(12) + Duration::hours(15),
                duration: Duration::days(2),
                importance: 9
            },
            Task {
                id: None,
                content: "Ask advice from Saruman".to_string(),
                deadline: Local::now() + Duration::days(8) + Duration::hours(15),
                duration: Duration::days(3),
                importance: 4
            },
            Task {
                id: None,
                content: "Visit Bilbo in Rivendel".to_string(),
                deadline: Local::now() + Duration::days(13) + Duration::hours(15),
                duration: Duration::days(2),
                importance: 2
            },
            Task {
                id: None,
                content: "Make some firework for the hobbits".to_string(),
                deadline: Local::now() + Duration::hours(33),
                duration: Duration::hours(3),
                importance: 3
            },
            Task {
                id: None,
                content: "Get riders of Rohan to help Gondor".to_string(),
                deadline: Local::now() + Duration::days(21) + Duration::hours(15),
                duration: Duration::days(7),
                importance: 7,
            },
            Task {
                id: None,
                content: "Find some good pipe-weed".to_string(),
                deadline: Local::now() + Duration::days(2) + Duration::hours(15),
                duration: Duration::hours(1),
                importance: 8
            },
            Task {
                id: None,
                content: "Go shop for white clothing".to_string(),
                deadline: Local::now() + Duration::days(33) + Duration::hours(15),
                duration: Duration::hours(2),
                importance: 3
            },
            Task {
                id: None,
                content: "Prepare epic-sounding one-liners".to_string(),
                deadline: Local::now() + Duration::hours(34),
                duration: Duration::hours(2),
                importance: 10
            },
            Task {
                id: None,
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
        let schedule = Schedule::schedule_according_to_importance(&tasks).unwrap();
        let mut expected_when = Local::now() + *SCHEDULE_DELAY;
        // 7. Prepare epic-sounding one-liners
        assert_eq!(*schedule.0[0].task, tasks[7]);
        assert!(are_approx_equal(schedule.0[0].when, expected_when));
        expected_when = expected_when + Duration::hours(2);
        // 5. Find some good pipe-weed
        assert_eq!(*schedule.0[1].task, tasks[5]);
        assert!(are_approx_equal(schedule.0[1].when, expected_when));
        expected_when = expected_when + Duration::hours(1);
        // 8. Recharge staff batteries
        assert_eq!(*schedule.0[2].task, tasks[8]);
        assert!(are_approx_equal(schedule.0[2].when, expected_when));
        expected_when = expected_when + Duration::minutes(30);
        // 3. Make some firework for the hobbits
        assert_eq!(*schedule.0[3].task, tasks[3]);
        assert!(are_approx_equal(schedule.0[3].when, expected_when));
        expected_when = expected_when + Duration::hours(3);
        // 0. Think of plan to get rid of The Ring
        assert_eq!(*schedule.0[4].task, tasks[0]);
        assert!(are_approx_equal(schedule.0[4].when, expected_when));
        expected_when = expected_when + Duration::days(2);
        // 1. Ask advice from Saruman
        assert_eq!(*schedule.0[5].task, tasks[1]);
        assert!(are_approx_equal(schedule.0[5].when, expected_when));
        expected_when = expected_when + Duration::days(3);
        // 6. Go shop for white clothing
        assert_eq!(*schedule.0[6].task, tasks[6]);
        assert!(are_approx_equal(schedule.0[6].when, expected_when));
        expected_when = expected_when + Duration::hours(2);
        // 2. Visit Bilbo in Rivendel
        assert_eq!(*schedule.0[7].task, tasks[2]);
        assert!(are_approx_equal(schedule.0[7].when, expected_when));
        expected_when = expected_when + Duration::days(2);
        // 4. Get riders of Rohan to help Gondor
        assert_eq!(*schedule.0[8].task, tasks[4]);
        assert!(are_approx_equal(schedule.0[8].when, expected_when));
    }

    fn taskset_with_missed_deadline() -> Vec<Task> {
        let task1 = Task {
            id: None,
            content: "conquer the world".to_string(),
            deadline: Local::now() + Duration::days(3),
            duration: Duration::days(1),
            importance: 5,
        };
        let task2 = Task {
            id: None,
            content: "save the world".to_string(),
            deadline: Local::now() - Duration::days(1),
            duration: Duration::minutes(5),
            importance: 5,
        };
        vec![task1, task2]
    }

    fn taskset_impossible() -> Vec<Task> {
        let task1 = Task {
            id: None,
            content: "Learn Rust".to_string(),
            deadline: Local::now() + Duration::days(1),
            duration: Duration::days(1),
            importance: 5,
        };
        let task2 = Task {
            id: None,
            content: "Program Eva".to_string(),
            deadline: Local::now() - Duration::days(2),
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
