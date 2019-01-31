use chrono::{DateTime, Duration, Utc};
use std::ops::Range;

pub trait TimeSegment: Clone {
    fn ranges(&self) -> &Vec<Range<DateTime<Utc>>>;
    fn start(&self) -> DateTime<Utc>;
    fn period(&self) -> Duration;

    /// Construct the inverse of the time segment, i.e. the time segment made up
    /// of all time that the given time segment _doesn't_ cover.
    fn inverse(&self) -> UnnamedTimeSegment {
        let mut ranges: Vec<Range<DateTime<Utc>>> = vec![];
        if self.ranges().len() > 0 {
            if self.ranges()[0].start - self.start() > Duration::seconds(0) {
                ranges.push(self.start()..self.ranges()[0].start);
            }
            for i in 0..self.ranges().len() - 1 {
                if self.ranges()[i + 1].start - self.ranges()[i].end > Duration::seconds(0) {
                    ranges.push(self.ranges()[i].end..self.ranges()[i + 1].start);
                }
            }
            if self.start() + self.period() - self.ranges()[self.ranges().len() - 1].end
                > Duration::seconds(0)
            {
                ranges
                    .push(self.ranges()[self.ranges().len() - 1].end..self.start() + self.period());
            }
        } else {
            ranges.push(self.start()..self.start() + self.period());
        }
        UnnamedTimeSegment {
            ranges,
            start: self.start(),
            period: self.period(),
        }
    }

    /// Generates all the time ranges that the time segment covers between the
    /// given start and end time.
    fn generate_ranges(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Vec<Range<DateTime<Utc>>> {
        let mut all_ranges = vec![];

        let mut period_start = start;
        let mut period_ranges = self.with_start(start).ranges().clone();

        while period_start < end {
            for mut range in &mut period_ranges {
                if range.start > end {
                    break;
                }
                all_ranges.push(range.clone());
                range.start = range.start + self.period();
                range.end = range.end + self.period();
            }
            period_start = period_start + self.period();
        }

        all_ranges
    }

    /// Returns a new time segment with its start and ranges shifted towards the
    /// given start time.
    fn with_start(&self, start: DateTime<Utc>) -> UnnamedTimeSegment {
        let diff = start - self.start();
        let ranges = self
            .ranges()
            .iter()
            .map(|range| range.start + diff..range.end + diff)
            .collect::<Vec<_>>();
        UnnamedTimeSegment {
            ranges,
            start,
            period: self.period(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct NamedTimeSegment {
    pub id: u32,
    pub name: String,
    // ranges is assumed to be in order
    pub ranges: Vec<Range<DateTime<Utc>>>,
    pub start: DateTime<Utc>,
    pub period: Duration,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnnamedTimeSegment {
    // ranges is assumed to be in order
    pub ranges: Vec<Range<DateTime<Utc>>>,
    pub start: DateTime<Utc>,
    pub period: Duration,
}

impl TimeSegment for NamedTimeSegment {
    fn ranges(&self) -> &Vec<Range<DateTime<Utc>>> {
        &self.ranges
    }

    fn start(&self) -> DateTime<Utc> {
        self.start
    }

    fn period(&self) -> Duration {
        self.period
    }
}

impl TimeSegment for UnnamedTimeSegment {
    fn ranges(&self) -> &Vec<Range<DateTime<Utc>>> {
        &self.ranges
    }

    fn start(&self) -> DateTime<Utc> {
        self.start
    }

    fn period(&self) -> Duration {
        self.period
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inverse_base_cases() {
        let start = Utc::now();
        let period = Duration::weeks(1);
        let anytime = UnnamedTimeSegment {
            ranges: vec![start..start + period],
            start,
            period,
        };
        let never = UnnamedTimeSegment {
            ranges: vec![],
            start,
            period,
        };
        assert_eq!(anytime.inverse(), never);
        assert_eq!(never.inverse(), anytime);
    }

    #[test]
    fn inverse_normal_segment() {
        let start = Utc::now();
        let period = Duration::weeks(1);
        let segment = UnnamedTimeSegment {
            ranges: vec![
                start + Duration::hours(24 + 10)..start + Duration::hours(24 + 15),
                start + Duration::hours(72 + 16)..start + Duration::hours(72 + 18),
                start + Duration::hours(72 + 19)..start + Duration::hours(72 + 21),
            ],
            start,
            period,
        };
        let inverse = UnnamedTimeSegment {
            ranges: vec![
                start..start + Duration::hours(24 + 10),
                start + Duration::hours(24 + 15)..start + Duration::hours(72 + 16),
                start + Duration::hours(72 + 18)..start + Duration::hours(72 + 19),
                start + Duration::hours(72 + 21)..start + period,
            ],
            start,
            period,
        };
        assert_eq!(segment.inverse(), inverse);
        assert_eq!(inverse.inverse(), segment);
    }

    #[test]
    fn generate_ranges_normal_cases() {
        fn normal_time_segment(start: DateTime<Utc>) -> impl TimeSegment {
            UnnamedTimeSegment {
                ranges: vec![
                    start + Duration::hours(24 + 10)..start + Duration::hours(24 + 15),
                    start + Duration::hours(72 + 16)..start + Duration::hours(72 + 18),
                    start + Duration::hours(72 + 19)..start + Duration::hours(72 + 21),
                ],
                start,
                period: Duration::weeks(1),
            }
        }

        let time0 = Utc::now();
        let time1 = Utc::now() + Duration::days(10);
        let time2 = Utc::now() + Duration::days(20);

        let segment = normal_time_segment(time0);
        assert_eq!(segment.generate_ranges(time0, time0), vec![]);
        assert_eq!(
            segment.generate_ranges(time0, time1),
            vec![
                time0 + Duration::hours(24 + 10)..time0 + Duration::hours(24 + 15),
                time0 + Duration::hours(72 + 16)..time0 + Duration::hours(72 + 18),
                time0 + Duration::hours(72 + 19)..time0 + Duration::hours(72 + 21),
                time0 + Duration::hours(7 * 24 + 24 + 10)
                    ..time0 + Duration::hours(7 * 24 + 24 + 15),
            ]
        );
        assert_eq!(segment.generate_ranges(time1, time0), vec![]);
        assert_eq!(segment.generate_ranges(time1, time1), vec![]);

        let segment = normal_time_segment(time2);
        assert_eq!(segment.generate_ranges(time0, time0), vec![]);
        assert_eq!(
            segment.generate_ranges(time0, time1),
            vec![
                time0 + Duration::hours(24 + 10)..time0 + Duration::hours(24 + 15),
                time0 + Duration::hours(72 + 16)..time0 + Duration::hours(72 + 18),
                time0 + Duration::hours(72 + 19)..time0 + Duration::hours(72 + 21),
                time0 + Duration::hours(7 * 24 + 24 + 10)
                    ..time0 + Duration::hours(7 * 24 + 24 + 15),
            ]
        );
        assert_eq!(
            segment.generate_ranges(time0, time2),
            vec![
                time0 + Duration::hours(24 + 10)..time0 + Duration::hours(24 + 15),
                time0 + Duration::hours(72 + 16)..time0 + Duration::hours(72 + 18),
                time0 + Duration::hours(72 + 19)..time0 + Duration::hours(72 + 21),
                time0 + Duration::hours(7 * 24 + 24 + 10)
                    ..time0 + Duration::hours(7 * 24 + 24 + 15),
                time0 + Duration::hours(7 * 24 + 72 + 16)
                    ..time0 + Duration::hours(7 * 24 + 72 + 18),
                time0 + Duration::hours(7 * 24 + 72 + 19)
                    ..time0 + Duration::hours(7 * 24 + 72 + 21),
                time0 + Duration::hours(14 * 24 + 24 + 10)
                    ..time0 + Duration::hours(14 * 24 + 24 + 15),
                time0 + Duration::hours(14 * 24 + 72 + 16)
                    ..time0 + Duration::hours(14 * 24 + 72 + 18),
                time0 + Duration::hours(14 * 24 + 72 + 19)
                    ..time0 + Duration::hours(14 * 24 + 72 + 21),
            ]
        );
        assert_eq!(segment.generate_ranges(time1, time0), vec![]);
        assert_eq!(segment.generate_ranges(time1, time1), vec![]);
    }

    #[test]
    fn with_start() {
        let start = Utc::now();
        let period = Duration::weeks(1);
        let segment = UnnamedTimeSegment {
            ranges: vec![
                start + Duration::hours(24 + 10)..start + Duration::hours(24 + 15),
                start + Duration::hours(72 + 16)..start + Duration::hours(72 + 18),
                start + Duration::hours(72 + 19)..start + Duration::hours(72 + 21),
            ],
            start,
            period,
        };
        let shifted_back_segment = UnnamedTimeSegment {
            ranges: vec![
                start + Duration::hours(10)..start + Duration::hours(15),
                start + Duration::hours(48 + 16)..start + Duration::hours(48 + 18),
                start + Duration::hours(48 + 19)..start + Duration::hours(48 + 21),
            ],
            start: start - Duration::days(1),
            period,
        };
        assert_eq!(
            segment.with_start(start - Duration::days(1)),
            shifted_back_segment
        );
        let shifted_forward_segment = UnnamedTimeSegment {
            ranges: vec![
                start + Duration::hours(48 + 10)..start + Duration::hours(48 + 15),
                start + Duration::hours(96 + 16)..start + Duration::hours(96 + 18),
                start + Duration::hours(96 + 19)..start + Duration::hours(96 + 21),
            ],
            start: start + Duration::days(1),
            period,
        };
        assert_eq!(
            segment.with_start(start + Duration::days(1)),
            shifted_forward_segment
        );
    }
}
