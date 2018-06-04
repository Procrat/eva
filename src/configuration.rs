use ::database::Database;


#[derive(Debug)]
pub struct Configuration {
    pub database: Box<Database>,
    pub scheduling_strategy: SchedulingStrategy,
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
