use cfg_if::cfg_if;
use chrono::{DateTime, Utc};

use crate::database::Database;

cfg_if! {
    if #[cfg(feature = "clock")] {
        #[derive(Debug)]
        pub struct Configuration {
            pub database: Box<Database>,
            pub scheduling_strategy: SchedulingStrategy,
        }
    } else {
        #[derive(Debug)]
        pub struct Configuration {
            pub database: Box<Database>,
            pub scheduling_strategy: SchedulingStrategy,
            pub time_context: Box<TimeContext>,
        }
    }
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

cfg_if! {
    if #[cfg(feature = "clock")] {
        impl Configuration {
            pub fn now(&self) -> DateTime<Utc> {
                Utc::now()
            }
        }
    } else {
        use std::fmt;

        pub trait TimeContext {
            fn now(&self) -> DateTime<Utc>;
        }

        impl fmt::Debug for TimeContext {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "<time context>")
            }
        }

        impl Configuration {
            pub fn now(&self) -> DateTime<Utc> {
                self.time_context.now()
            }
        }
    }
}
