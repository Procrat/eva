use cfg_if::cfg_if;
use chrono::{DateTime, Utc};

use crate::database::Database;

cfg_if! {
    if #[cfg(feature = "clock")] {
        #[derive(Debug)]
        pub struct Configuration {
            pub database: Box<dyn Database>,
            pub scheduling_strategy: SchedulingStrategy,
        }
    } else {
        #[derive(Debug)]
        pub struct Configuration {
            pub database: Box<dyn Database>,
            pub scheduling_strategy: SchedulingStrategy,
            pub time_context: Box<dyn TimeContext>,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum SchedulingStrategy {
    Importance,
    Urgency,
}

impl SchedulingStrategy {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Importance => "importance",
            Self::Urgency => "urgency",
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

        impl fmt::Debug for dyn TimeContext {
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
