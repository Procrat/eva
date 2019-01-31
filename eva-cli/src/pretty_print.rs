use chrono::prelude::*;
use itertools::Itertools;

pub(crate) trait PrettyPrint {
    fn pretty_print(&self) -> String;
}

impl PrettyPrint for eva::Schedule<eva::Task> {
    fn pretty_print(&self) -> String {
        format!(
            "Schedule:\n  {}",
            self.0.iter().map(PrettyPrint::pretty_print).join("\n  ")
        )
    }
}

impl PrettyPrint for eva::Scheduled<eva::Task> {
    fn pretty_print(&self) -> String {
        format!("{}: {}", self.when.pretty_print(), self.task.pretty_print())
    }
}

impl PrettyPrint for DateTime<Utc> {
    fn pretty_print(&self) -> String {
        let format = if self.year() == Utc::now().year() {
            "%a %-d %b %-H:%M"
        } else {
            "%a %-d %b %Y %-H:%M"
        };
        self.format(format).to_string()
    }
}

impl PrettyPrint for eva::Task {
    fn pretty_print(&self) -> String {
        let prefix = format!("{}. ", self.id);
        format!(
            "{}{}\n{}(deadline: {}, duration: {}, importance: {})",
            prefix,
            self.content,
            " ".repeat(prefix.len()),
            self.deadline.pretty_print(),
            self.duration.pretty_print(),
            self.importance
        )
    }
}

impl PrettyPrint for chrono::Duration {
    fn pretty_print(&self) -> String {
        if self.num_minutes() > 0 {
            format!("{}h{}", self.num_hours(), self.num_minutes() % 60)
        } else {
            format!("{}h", self.num_hours())
        }
    }
}
