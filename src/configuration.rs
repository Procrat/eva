use std::fmt;

use chrono::{DateTime, Local};

use ::database::Database;


#[derive(Debug)]
pub struct Configuration {
    pub database: Box<Database>,
    pub scheduling_strategy: SchedulingStrategy,
    pub time_context: Option<Box<TimeContext>>,
}


#[derive(Debug)]
pub enum SchedulingStrategy {
    Importance,
    Urgency,
}

impl SchedulingStrategy {
    pub fn as_str(&self) -> &'static str {
        match *self {
            SchedulingStrategy::Importance => "importance",
            SchedulingStrategy::Urgency => "urgency",
        }
    }
}


pub trait TimeContext {
    fn now(&self) -> DateTime<Local>;
}

impl fmt::Debug for TimeContext {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<time context>")
    }
}

impl TimeContext for Local {
    fn now(&self) -> DateTime<Local> {
        Local::now()
    }
}
