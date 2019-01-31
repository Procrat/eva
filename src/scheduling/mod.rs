use std::fmt::{self, Debug, Display};
use std::hash::Hash;
use std::rc::Rc;

use chrono::prelude::*;
use chrono::Duration;
use failure::Fail;
use itertools::Itertools;

use crate::configuration::SchedulingStrategy;
use crate::time_segment::TimeSegment;

use self::schedule_tree::{Entry, ScheduleTree};

mod schedule_tree;

pub(crate) trait Task:
    Debug + Display + Send + Sync + PartialEq + Eq + Clone + Hash
{
    fn deadline(&self) -> DateTime<Utc>;
    fn duration(&self) -> Duration;
    fn importance(&self) -> u32;
}

impl Task for crate::Task {
    fn deadline(&self) -> DateTime<Utc> {
        self.deadline
    }

    fn duration(&self) -> Duration {
        self.duration
    }

    fn importance(&self) -> u32 {
        self.importance
    }
}

#[derive(Debug, Fail)]
pub enum Error<TaskT: Debug + Display + Send + Sync + 'static> {
    #[fail(
        display = "I could not schedule {} because you {} the deadline.\nYou might want to \
                   postpone this task or remove it if it's not longer relevant",
        task, tense
    )]
    DeadlineMissed { task: TaskT, tense: &'static str },
    #[fail(
        display = "I could not schedule {} because you don't have enough time to do \
                   everything.\nYou might want to decide not to do some things or relax their \
                   deadlines",
        task
    )]
    NotEnoughTime { task: TaskT },
    #[fail(
        display = "An internal error occurred (This shouldn't happen.): {}",
        _0
    )]
    Internal(&'static str),
}

#[derive(Debug, PartialEq)]
pub struct Scheduled<T> {
    pub task: T,
    pub when: DateTime<Utc>,
}

impl<TaskT: PartialEq> std::cmp::PartialOrd for Scheduled<TaskT> {
    fn partial_cmp(&self, other: &Scheduled<TaskT>) -> Option<std::cmp::Ordering> {
        match self.when.cmp(&other.when) {
            std::cmp::Ordering::Equal => None,
            strict_ordering => Some(strict_ordering),
        }
    }
}

#[derive(Debug)]
pub struct Schedule<TaskT>(pub Vec<Scheduled<TaskT>>);

impl<TaskT> Default for Schedule<TaskT> {
    fn default() -> Self {
        Schedule(vec![])
    }
}

impl<TaskT> Schedule<TaskT> {
    /// Schedules tasks according to the given strategy, using the tasks'
    /// deadlines, importance and duration.
    ///
    /// Args:
    ///     start: the moment when the first task can be scheduled
    ///     tasks: iterable of tasks to schedule
    ///     strategy: the scheduling algorithm to use
    ///     time_segment: the time segment to schedule the tasks within
    /// Returns when successful an instance of Schedule which contains all
    /// tasks, each bound to a certain date and time; returns None when not all
    /// tasks could be scheduled.
    pub(crate) fn schedule(
        start: DateTime<Utc>,
        tasks_per_segment: impl IntoIterator<Item = (impl TimeSegment, impl IntoIterator<Item = TaskT>)>,
        strategy: SchedulingStrategy,
    ) -> Result<Schedule<TaskT>, Error<TaskT>>
    where
        TaskT: Task,
    {
        tasks_per_segment
            .into_iter()
            .map(|(segment, tasks)| {
                Schedule::schedule_within_segment(start, tasks, segment, strategy)
            })
            .fold(
                Ok(Schedule::default()),
                |acc_schedule, new_schedule| match (acc_schedule, new_schedule) {
                    (Err(error), _) => Err(error),
                    (_, Err(error)) => Err(error),
                    (Ok(acc_schedule), Ok(new_schedule)) => Ok(Schedule(
                        itertools::merge(acc_schedule.0, new_schedule.0).collect_vec(),
                    )),
                },
            )
    }

    fn schedule_within_segment(
        start: DateTime<Utc>,
        tasks: impl IntoIterator<Item = TaskT>,
        segment: impl TimeSegment,
        strategy: SchedulingStrategy,
    ) -> Result<Schedule<TaskT>, Error<TaskT>>
    where
        TaskT: Task,
    {
        let tasks: Vec<Rc<TaskT>> = tasks.into_iter().map(Rc::new).collect();
        if tasks.is_empty() {
            Ok(Schedule::default())
        } else {
            let mut tree: ScheduleTree<DateTime<Utc>, Item<TaskT>> = ScheduleTree::new();
            // Make sure things aren't scheduled before the algorithm is finished.
            let last_deadline = tasks
                .iter()
                .map(|task| task.deadline())
                .max()
                .ok_or(Error::Internal("last deadline not found"))?;
            let unscheduleables = segment.inverse().generate_ranges(start, last_deadline);
            for unscheduleable in unscheduleables {
                tree.schedule_exact(
                    unscheduleable.start,
                    unscheduleable.end - unscheduleable.start,
                    Item::Nothing,
                );
            }
            match strategy {
                SchedulingStrategy::Importance => {
                    tree.schedule_according_to_importance(start, tasks)
                }
                SchedulingStrategy::Urgency => tree.schedule_according_to_myrjam(start, tasks),
            }?;
            Ok(Schedule::from_tree(tree))
        }
    }

    fn from_tree(tree: ScheduleTree<DateTime<Utc>, Item<TaskT>>) -> Schedule<TaskT>
    where
        TaskT: Task,
    {
        let scheduled_tasks = tree
            .into_iter()
            .filter_map(|entry| match entry.data {
                Item::Nothing => None,
                Item::Task(task) => Some(Scheduled {
                    task: (*task).clone(),
                    when: entry.start,
                }),
            })
            .collect();
        Schedule(scheduled_tasks)
    }
}

#[derive(Debug, Hash, Clone)]
enum Item<TaskT> {
    Task(Rc<TaskT>),
    Nothing,
}

impl<TaskT: PartialEq> PartialEq for Item<TaskT> {
    fn eq(&self, other: &Item<TaskT>) -> bool {
        match (self, other) {
            (Item::Task(task), Item::Task(other)) => task.eq(other),
            _ => false,
        }
    }
}

// HACK: We're lying here. According to our implementation of PartialEq, the
// equivalence relation not reflexive for Nothing. The ScheduleTree needs it for
// its internal hash map which it uses for data lookups. So this hack will cause
// e.g. all Nothings to be un-unscheduleable.
impl<TaskT: PartialEq> Eq for Item<TaskT> {}

trait Scheduler<TaskT: Task> {
    fn schedule_according_to_importance(
        &mut self,
        start: DateTime<Utc>,
        tasks: Vec<Rc<TaskT>>,
    ) -> Result<(), Error<TaskT>>;
    fn schedule_according_to_myrjam(
        &mut self,
        start: DateTime<Utc>,
        tasks: Vec<Rc<TaskT>>,
    ) -> Result<(), Error<TaskT>>;
}

impl<TaskT: Task> Scheduler<TaskT> for ScheduleTree<DateTime<Utc>, Item<TaskT>> {
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
    fn schedule_according_to_importance(
        &mut self,
        start: DateTime<Utc>,
        mut tasks: Vec<Rc<TaskT>>,
    ) -> Result<(), Error<TaskT>> {
        // Start by scheduling the least important tasks closest to the deadline, and so on.
        tasks.sort_by_key(|task| {
            (
                task.importance(),
                start.signed_duration_since(task.deadline()),
            )
        });
        for task in &tasks {
            if task.deadline() < start + task.duration() {
                return Err(Error::DeadlineMissed {
                    task: (**task).clone(),
                    tense: if task.deadline() < start {
                        "missed"
                    } else {
                        "will miss"
                    },
                });
            }
            if !self.schedule_close_before(
                task.deadline(),
                task.duration(),
                Some(start),
                Item::Task(Rc::clone(task)),
            ) {
                return Err(Error::NotEnoughTime {
                    task: (**task).clone(),
                });
            }
        }
        // Next, shift the most important tasks towards today, and so on, filling up the gaps.
        // Keep repeating that, until nothing changes anymore (i.e. all gaps are filled).
        let mut changed = !self.is_empty();
        while changed {
            changed = false;
            for task in tasks.iter().rev() {
                let scheduled_entry = self
                    .unschedule(&Item::Task(task.clone()))
                    .ok_or_else(|| Error::Internal("I couldn't unschedule a task"))?;
                if !self.schedule_close_after(
                    start,
                    task.duration(),
                    Some(scheduled_entry.end),
                    scheduled_entry.data,
                ) {
                    return Err(Error::Internal("I couldn't reschedule a task"));
                }
                let new_start =
                    self.when_scheduled(&Item::Task(task.clone()))
                        .ok_or_else(|| {
                            Error::Internal("I couldn't find a task that was just scheduled")
                        })?;
                if scheduled_entry.start != *new_start {
                    changed = true;
                    break;
                }
            }
        }
        Ok(())
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
    fn schedule_according_to_myrjam(
        &mut self,
        start: DateTime<Utc>,
        mut tasks: Vec<Rc<TaskT>>,
    ) -> Result<(), Error<TaskT>> {
        // Start by scheduling the least important tasks closest to the deadline, and so on.
        tasks.sort_by_key(|task| task.importance());
        for task in tasks {
            if task.deadline() < start + task.duration() {
                return Err(Error::DeadlineMissed {
                    task: (*task).clone(),
                    tense: if task.deadline() < start {
                        "missed"
                    } else {
                        "will miss"
                    },
                });
            }
            if !self.schedule_close_before(
                task.deadline(),
                task.duration(),
                Some(start),
                Item::Task(Rc::clone(&task)),
            ) {
                return Err(Error::NotEnoughTime {
                    task: (*task).clone(),
                });
            }
        }
        // Next, shift the all tasks towards the present, filling up the gaps.
        let entries = self
            .iter()
            .map(|entry| Entry {
                start: entry.start,
                end: entry.end,
                data: (*entry.data).clone(),
            })
            .collect::<Vec<_>>();
        for entry in entries {
            if let Item::Task(ref task) = entry.data {
                let scheduled_entry = self
                    .unschedule(&entry.data)
                    .ok_or_else(|| Error::Internal("I couldn't unschedule a task"))?;
                if !self.schedule_close_after(
                    start,
                    task.duration(),
                    Some(scheduled_entry.end),
                    scheduled_entry.data,
                ) {
                    return Err(Error::Internal("I couldn't reschedule a task"));
                }
            }
        }
        Ok(())
    }
}

impl fmt::Display for crate::Task {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.content)
    }
}

#[cfg(test)]
mod tests {
    use assert_matches::assert_matches;
    use chrono::Duration;

    use super::*;
    use crate::time_segment::UnnamedTimeSegment;

    #[derive(Debug, PartialEq, Eq, Clone, Hash)]
    struct Task {
        pub content: String,
        pub deadline: DateTime<Utc>,
        pub duration: Duration,
        pub importance: u32,
    }

    impl super::Task for Task {
        fn deadline(&self) -> DateTime<Utc> {
            self.deadline
        }

        fn duration(&self) -> Duration {
            self.duration
        }

        fn importance(&self) -> u32 {
            self.importance
        }
    }

    impl Display for Task {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "{}", self.content)
        }
    }

    type Result<T> = std::result::Result<T, Error<Task>>;

    fn anytime() -> impl TimeSegment {
        let start = Utc::now();
        let period = Duration::weeks(1);
        UnnamedTimeSegment {
            ranges: vec![start..start + period],
            start,
            period,
        }
    }

    fn never() -> impl TimeSegment {
        UnnamedTimeSegment {
            ranges: vec![],
            start: Utc::now(),
            period: Duration::weeks(1),
        }
    }

    macro_rules! test_generic_properties {
        ($($strategy_name:ident: $strategy:expr,)*) => {
            $(
                mod $strategy_name {
                    use super::*;

                    /// Schedules the given tasks in a time segment without
                    /// gaps.
                    fn schedule(tasks: Vec<Task>, start: DateTime<Utc>) -> Result<Schedule<Task>> {
                        Schedule::schedule_within_segment(start, tasks, anytime(), $strategy)
                    }

                    #[test]
                    fn all_tasks_are_scheduled() {
                        let start = Utc::now();
                        for tasks in vec![taskset_of_myrjam(), taskset_just_in_time(start)] {
                            let schedule = schedule(tasks.clone(), start).unwrap();
                            assert_eq!(tasks.len(), schedule.0.len());
                            for scheduled_task in schedule.0.iter() {
                                assert!(tasks.contains(&scheduled_task.task));
                            }
                            for task in tasks {
                                assert!(schedule.0.iter()
                                        .any(|scheduled_task| scheduled_task.task == task));
                            }
                        }
                    }

                    #[test]
                    fn tasks_are_in_scheduled_in_time() {
                        let start = Utc::now();
                        for tasks in vec![taskset_of_myrjam(), taskset_just_in_time(start)] {
                            let schedule = schedule(tasks, start).unwrap();
                            for scheduled_task in schedule.0.iter() {
                                assert!(scheduled_task.when <= scheduled_task.task.deadline);
                            }
                        }
                    }

                    #[test]
                    fn schedule_just_in_time() {
                        let start = Utc::now();
                        let tasks = taskset_just_in_time(start);
                        let schedule = schedule(tasks.clone(), start).unwrap();
                        assert_eq!(schedule.0[0].task, tasks[0]);
                        assert_eq!(schedule.0[1].task, tasks[1]);
                        assert_eq!(schedule.0[0].when, start);
                        assert_eq!(schedule.0[1].when, start + Duration::days(23 * 365));
                    }

                    #[test]
                    fn schedule_sets_of_two() {
                        let start = Utc::now();
                        let mut tasks = vec![Task {
                            content: "find meaning to life".to_string(),
                            deadline: start + Duration::hours(1),
                            duration: Duration::hours(1),
                            importance: 6,
                        },
                        Task {
                            content: "stop giving a fuck".to_string(),
                            deadline: start + Duration::hours(3),
                            duration: Duration::hours(2),
                            importance: 5,
                        }];
                        // Normal scheduling
                        {
                            let schedule = schedule(tasks.clone(), start).unwrap();
                            assert_eq!(schedule.0[0].task, tasks[0]);
                            assert_eq!(schedule.0[1].task, tasks[1]);
                        }

                        // Reversing the importance should maintain the scheduled order, because it's the only way
                        // to meet the deadlines.
                        tasks[0].importance = 5;
                        tasks[1].importance = 6;
                        {
                            let schedule = schedule(tasks.clone(), start).unwrap();
                            assert_eq!(schedule.0[0].task, tasks[0]);
                            assert_eq!(schedule.0[1].task, tasks[1]);
                        }

                        // Leveling the deadlines should make the more important task be scheduled first again.
                        tasks[0].deadline = start + Duration::hours(3);
                        let schedule = schedule(tasks.clone(), start).unwrap();
                        assert_eq!(schedule.0[0].task, tasks[1]);
                        assert_eq!(schedule.0[1].task, tasks[0]);
                    }

                    #[test]
                    fn no_schedule() {
                        let tasks = vec![];
                        let schedule = schedule(tasks, Utc::now()).unwrap();
                        assert!(schedule.0.is_empty());
                    }

                    #[test]
                    fn missed_deadline() {
                        let tasks = taskset_with_missed_deadline();
                        assert_matches!(schedule(tasks, Utc::now()),
                                        Err(Error::DeadlineMissed { tense, .. })
                                        if tense == "missed");
                    }

                    #[test]
                    fn impossible_deadline() {
                        let tasks = taskset_with_impossible_deadline();
                        assert_matches!(schedule(tasks, Utc::now()),
                                        Err(Error::DeadlineMissed { tense, .. })
                                        if tense == "will miss");
                    }

                    #[test]
                    fn out_of_time() {
                        let start = Utc::now();
                        let tasks = taskset_impossible_combination(start);
                        assert_matches!(schedule(tasks, start),
                                        Err(Error::NotEnoughTime { .. }));
                    }

                    #[test]
                    fn schedules_within_the_time_segment() {
                        let now = Utc::now();
                        let tasks = vec![
                            Task {
                                content: "urgent-quick".to_string(),
                                deadline: now + Duration::days(2),
                                duration: Duration::minutes(20),
                                importance: 4,
                            },
                            Task {
                                content: "important-quick".to_string(),
                                deadline: now + Duration::days(2),
                                duration: Duration::minutes(20),
                                importance: 9,
                            },
                            Task {
                                content: "urgent-long".to_string(),
                                deadline: now + Duration::days(4),
                                duration: Duration::hours(2),
                                importance: 4,
                            },
                            Task {
                                content: "important-long".to_string(),
                                deadline: now + Duration::days(4),
                                duration: Duration::hours(2),
                                importance: 9,
                            },
                        ];
                        let segment = UnnamedTimeSegment {
                            ranges: vec![now + Duration::hours(10)..now + Duration::hours(12)],
                            start: now,
                            period: Duration::days(1),
                        };
                        let schedule = Schedule::schedule_within_segment(now, tasks, segment, $strategy);
                        assert_matches!(schedule, Ok(Schedule(scheduled_tasks)) => {
                            for scheduled_task in scheduled_tasks {
                                let start = scheduled_task.when;
                                let end = scheduled_task.when + scheduled_task.task.duration;
                                assert!(
                                    (start >= now + Duration::hours(10)
                                     && end <= now + Duration::hours(12))
                                        || (start >= now + Duration::days(1) + Duration::hours(10)
                                            && end <= now + Duration::days(1) + Duration::hours(12))
                                        || (start >= now + Duration::days(2) + Duration::hours(10)
                                            && end <= now + Duration::days(2) + Duration::hours(12))
                                );
                            }
                        });
                    }

                    #[test]
                    fn fails_if_no_space_in_time_segment() {
                        let now = Utc::now();
                        // Segment: two hours daily
                        let segment = UnnamedTimeSegment {
                            ranges: vec![now + Duration::hours(10)..now + Duration::hours(12)],
                            start: now,
                            period: Duration::days(1),
                        };

                        // Trying to schedule tasks longer than two hours fails
                        let tasks = vec![
                            Task {
                                content: "too-long".to_string(),
                                deadline: now + Duration::days(4),
                                duration: Duration::hours(2) + Duration::seconds(1),
                                importance: 10,
                            },
                        ];
                        let schedule = Schedule::schedule_within_segment(now, tasks, segment.clone(), $strategy);
                        assert_matches!(schedule, Err(Error::NotEnoughTime { .. }));

                        // Trying to schedule more tasks than possible to fit in
                        // to the segment, fails as well
                        let tasks = vec![
                            Task {
                                content: "task1".to_string(),
                                deadline: now + Duration::hours(36) - Duration::seconds(1),
                                duration: Duration::hours(1),
                                importance: 5,
                            },
                            Task {
                                content: "task2".to_string(),
                                deadline: now + Duration::hours(36) - Duration::seconds(1),
                                duration: Duration::hours(1),
                                importance: 5,
                            },
                            Task {
                                content: "task3".to_string(),
                                deadline: now + Duration::hours(36) - Duration::seconds(1),
                                duration: Duration::hours(2),
                                importance: 5,
                            },
                        ];
                        let schedule = Schedule::schedule_within_segment(now, tasks, segment, $strategy);
                        assert_matches!(schedule, Err(Error::NotEnoughTime { .. }));
                    }

                    #[test]
                    fn can_handle_never_time_segment() {
                        let tasks = taskset_of_myrjam();
                        let schedule = Schedule::schedule_within_segment(Utc::now(), tasks, never(), $strategy);
                        assert_matches!(schedule, Err(Error::NotEnoughTime { .. }));
                        let tasks: Vec<Task> = vec![];
                        let schedule = Schedule::schedule_within_segment(Utc::now(), tasks, never(), $strategy);
                        assert_matches!(schedule, Ok(Schedule(ref tasks)) if tasks.is_empty());
                    }
                }
             )*
        }
    }

    test_generic_properties! {
        importance: SchedulingStrategy::Importance,
        urgency: SchedulingStrategy::Urgency,
    }

    // Note that some of these task sets are not representative at all, since tasks should be small
    // and actionable. Things like taking over the world should be handled by Eva in a higher
    // abstraction level in something like projects, which should not be scheduled.

    fn taskset_of_myrjam() -> Vec<Task> {
        let now = Utc::now();
        let task1 = Task {
            content: "take over the world".to_string(),
            deadline: now + Duration::days(6 * 365),
            duration: Duration::hours(1000),
            importance: 10,
        };
        let task2 = Task {
            content: "make onion soup".to_string(),
            deadline: now + Duration::hours(2),
            duration: Duration::hours(1),
            importance: 3,
        };
        let task3 = Task {
            content: "publish Commander Mango 3".to_string(),
            deadline: now + Duration::days(365 / 2),
            duration: Duration::hours(50),
            importance: 6,
        };
        let task4 = Task {
            content: "sculpt".to_string(),
            deadline: now + Duration::days(30),
            duration: Duration::hours(10),
            importance: 4,
        };
        let task5 = Task {
            content: "organise birthday present".to_string(),
            deadline: now + Duration::days(30),
            duration: Duration::hours(5),
            importance: 10,
        };
        let task6 = Task {
            content: "make dentist appointment".to_string(),
            deadline: now + Duration::days(7),
            duration: Duration::minutes(10),
            importance: 5,
        };
        vec![task1, task2, task3, task4, task5, task6]
    }

    fn taskset_just_in_time(now: DateTime<Utc>) -> Vec<Task> {
        let task1 = Task {
            content: "go to school".to_string(),
            deadline: now + Duration::days(23 * 365),
            duration: Duration::days(23 * 365),
            importance: 5,
        };
        let task2 = Task {
            content: "work till you die".to_string(),
            deadline: now + Duration::days(65 * 365),
            duration: Duration::days(42 * 365),
            importance: 6,
        };
        vec![task1, task2]
    }

    #[test]
    fn schedule_for_myrjam() {
        let tasks = taskset_of_myrjam();
        let start = Utc::now();
        let schedule = Schedule::schedule_within_segment(
            start,
            tasks.clone(),
            anytime(),
            SchedulingStrategy::Urgency,
        )
        .unwrap();
        let mut expected_when = start;
        // 1. Make onion soup, 1h, 3, in 2 hours
        assert_eq!(schedule.0[0].task, tasks[1]);
        assert_eq!(schedule.0[0].when, expected_when);
        expected_when = expected_when + Duration::hours(1);
        // 5. Make dentist appointment, 10m, 5, in 7 days
        assert_eq!(schedule.0[1].task, tasks[5]);
        assert_eq!(schedule.0[1].when, expected_when);
        expected_when = expected_when + Duration::minutes(10);
        // 4. Organise birthday present, 5h, 10, in 30 days
        assert_eq!(schedule.0[2].task, tasks[4]);
        assert_eq!(schedule.0[2].when, expected_when);
        expected_when = expected_when + Duration::hours(5);
        // 3. Sculpt, 10h, 4, in 30 days
        assert_eq!(schedule.0[3].task, tasks[3]);
        assert_eq!(schedule.0[3].when, expected_when);
        expected_when = expected_when + Duration::hours(10);
        // 2. Public Commander Mango 3, 50h, 6, in 6 months
        assert_eq!(schedule.0[4].task, tasks[2]);
        assert_eq!(schedule.0[4].when, expected_when);
        expected_when = expected_when + Duration::hours(50);
        // 0. Take over world, 1000h, 10, in 10 years
        assert_eq!(schedule.0[5].task, tasks[0]);
        assert_eq!(schedule.0[5].when, expected_when);
    }

    #[test]
    fn schedule_myrjams_schedule_by_importance() {
        let tasks = taskset_of_myrjam();
        let start = Utc::now();
        let schedule = Schedule::schedule_within_segment(
            start,
            tasks.clone(),
            anytime(),
            SchedulingStrategy::Importance,
        )
        .unwrap();
        let mut expected_when = start;
        // 5. Make dentist appointment, 10m, 5, in 7 days
        assert_eq!(schedule.0[0].task, tasks[5]);
        assert_eq!(schedule.0[0].when, expected_when);
        expected_when = expected_when + Duration::minutes(10);
        // 1. Make onion soup, 1h, 3, in 2 hours
        assert_eq!(schedule.0[1].task, tasks[1]);
        assert_eq!(schedule.0[1].when, expected_when);
        expected_when = expected_when + Duration::hours(1);
        // 4. Organise birthday present, 5h, 10, in 30 days
        assert_eq!(schedule.0[2].task, tasks[4]);
        assert_eq!(schedule.0[2].when, expected_when);
        expected_when = expected_when + Duration::hours(5);
        // 2. Public Commander Mango 3, 50h, 6, in 6 months
        assert_eq!(schedule.0[3].task, tasks[2]);
        assert_eq!(schedule.0[3].when, expected_when);
        expected_when = expected_when + Duration::hours(50);
        // 3. Sculpt, 10h, 4, in 30 days
        assert_eq!(schedule.0[4].task, tasks[3]);
        assert_eq!(schedule.0[4].when, expected_when);
        expected_when = expected_when + Duration::hours(10);
        // 0. Take over world, 1000h, 10, in 10 years
        assert_eq!(schedule.0[5].task, tasks[0]);
        assert_eq!(schedule.0[5].when, expected_when);
    }

    fn taskset_of_gandalf() -> Vec<Task> {
        let now = Utc::now();
        vec![
            Task {
                content: "Think of plan to get rid of The Ring".to_string(),
                deadline: now + Duration::days(12) + Duration::hours(15),
                duration: Duration::days(2),
                importance: 9,
            },
            Task {
                content: "Ask advice from Saruman".to_string(),
                deadline: now + Duration::days(8) + Duration::hours(15),
                duration: Duration::days(3),
                importance: 4,
            },
            Task {
                content: "Visit Bilbo in Rivendel".to_string(),
                deadline: now + Duration::days(13) + Duration::hours(15),
                duration: Duration::days(2),
                importance: 2,
            },
            Task {
                content: "Make some firework for the hobbits".to_string(),
                deadline: now + Duration::hours(33),
                duration: Duration::hours(3),
                importance: 3,
            },
            Task {
                content: "Get riders of Rohan to help Gondor".to_string(),
                deadline: now + Duration::days(21) + Duration::hours(15),
                duration: Duration::days(7),
                importance: 7,
            },
            Task {
                content: "Find some good pipe-weed".to_string(),
                deadline: now + Duration::days(2) + Duration::hours(15),
                duration: Duration::hours(1),
                importance: 8,
            },
            Task {
                content: "Go shop for white clothing".to_string(),
                deadline: now + Duration::days(33) + Duration::hours(15),
                duration: Duration::hours(2),
                importance: 3,
            },
            Task {
                content: "Prepare epic-sounding one-liners".to_string(),
                deadline: now + Duration::hours(34),
                duration: Duration::hours(2),
                importance: 10,
            },
            Task {
                content: "Recharge staff batteries".to_string(),
                deadline: now + Duration::days(1) + Duration::hours(15),
                duration: Duration::minutes(30),
                importance: 5,
            },
        ]
    }

    #[test]
    fn schedule_gandalfs_schedule_by_importance() {
        let tasks = taskset_of_gandalf();
        let start = Utc::now();
        let schedule = Schedule::schedule_within_segment(
            start,
            tasks.clone(),
            anytime(),
            SchedulingStrategy::Importance,
        )
        .unwrap();
        let mut expected_when = start;
        // 7. Prepare epic-sounding one-liners
        assert_eq!(schedule.0[0].task, tasks[7]);
        assert_eq!(schedule.0[0].when, expected_when);
        expected_when = expected_when + Duration::hours(2);
        // 5. Find some good pipe-weed
        assert_eq!(schedule.0[1].task, tasks[5]);
        assert_eq!(schedule.0[1].when, expected_when);
        expected_when = expected_when + Duration::hours(1);
        // 8. Recharge staff batteries
        assert_eq!(schedule.0[2].task, tasks[8]);
        assert_eq!(schedule.0[2].when, expected_when);
        expected_when = expected_when + Duration::minutes(30);
        // 3. Make some firework for the hobbits
        assert_eq!(schedule.0[3].task, tasks[3]);
        assert_eq!(schedule.0[3].when, expected_when);
        expected_when = expected_when + Duration::hours(3);
        // 0. Think of plan to get rid of The Ring
        assert_eq!(schedule.0[4].task, tasks[0]);
        assert_eq!(schedule.0[4].when, expected_when);
        expected_when = expected_when + Duration::days(2);
        // 1. Ask advice from Saruman
        assert_eq!(schedule.0[5].task, tasks[1]);
        assert_eq!(schedule.0[5].when, expected_when);
        expected_when = expected_when + Duration::days(3);
        // 6. Go shop for white clothing
        assert_eq!(schedule.0[6].task, tasks[6]);
        assert_eq!(schedule.0[6].when, expected_when);
        expected_when = expected_when + Duration::hours(2);
        // 2. Visit Bilbo in Rivendel
        assert_eq!(schedule.0[7].task, tasks[2]);
        assert_eq!(schedule.0[7].when, expected_when);
        expected_when = expected_when + Duration::days(2);
        // 4. Get riders of Rohan to help Gondor
        assert_eq!(schedule.0[8].task, tasks[4]);
        assert_eq!(schedule.0[8].when, expected_when);
    }

    fn taskset_with_missed_deadline() -> Vec<Task> {
        let task1 = Task {
            content: "conquer the world".to_string(),
            deadline: Utc::now() + Duration::days(3),
            duration: Duration::days(1),
            importance: 5,
        };
        let task2 = Task {
            content: "save the world".to_string(),
            deadline: Utc::now() - Duration::days(1),
            duration: Duration::minutes(5),
            importance: 5,
        };
        vec![task1, task2]
    }

    fn taskset_with_impossible_deadline() -> Vec<Task> {
        let task1 = Task {
            content: "conquer the world".to_string(),
            deadline: Utc::now() + Duration::days(3),
            duration: Duration::days(1),
            importance: 5,
        };
        let task2 = Task {
            content: "save the world".to_string(),
            deadline: Utc::now() + Duration::hours(23),
            duration: Duration::days(1),
            importance: 5,
        };
        vec![task1, task2]
    }

    fn taskset_impossible_combination(now: DateTime<Utc>) -> Vec<Task> {
        let task1 = Task {
            content: "Learn Rust".to_string(),
            deadline: now + Duration::days(1),
            duration: Duration::days(1),
            importance: 5,
        };
        let task2 = Task {
            content: "Program Eva".to_string(),
            deadline: now + Duration::days(2),
            duration: Duration::days(1) + Duration::minutes(1),
            importance: 5,
        };
        vec![task1, task2]
    }
}
