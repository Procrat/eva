use std::ops::Range;

use chrono::{DateTime, Duration, Utc};
use itertools::Itertools;

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
        let mut all_ranges: Vec<Range<_>> = vec![];

        let mut period_start = start;
        let mut period_ranges = self.with_start(start).ranges().clone();

        while period_start < end {
            for mut range in &mut period_ranges {
                if range.start > end {
                    break;
                }
                // The temporary boolean here is necessary here since we don't have "if let" chains
                // at the time of writing
                let mut added = false;
                if let Some(last) = all_ranges.last_mut() {
                    if last.end == range.start {
                        last.end = range.end;
                        added = true;
                    }
                }
                if !added {
                    if range.end > end {
                        all_ranges.push(range.start..end);
                        break;
                    } else {
                        all_ranges.push(range.clone());
                    }
                }
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
        let shift = |datetime: DateTime<Utc>| -> DateTime<Utc> {
            let diff_ns = (datetime - start)
                .num_nanoseconds()
                .expect("300 years is a long time");
            let period_ns = self
                .period()
                .num_nanoseconds()
                .expect("300 years is a long time");
            let quotient = if diff_ns < 0 {
                diff_ns / period_ns - 1
            } else {
                diff_ns / period_ns
            };
            datetime - Duration::nanoseconds(quotient * period_ns)
        };
        let ranges = self
            .ranges()
            .iter()
            .map(|range| {
                let start = shift(range.start);
                let end = start + (range.end - range.start);
                start..end
            })
            .sorted_by_key(|range| range.start)
            .flat_map(|range| {
                if range.end <= start + self.period() {
                    vec![range]
                } else {
                    vec![
                        range.start..start + self.period(),
                        start..range.end - self.period(),
                    ]
                }
            })
            .sorted_by_key(|range| range.start)
            .collect();
        UnnamedTimeSegment {
            ranges,
            start,
            period: self.period(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NamedTimeSegment {
    pub id: u32,
    pub name: String,
    // ranges is assumed to be in order
    pub ranges: Vec<Range<DateTime<Utc>>>,
    pub start: DateTime<Utc>,
    pub period: Duration,
    pub hue: u16,
}

#[derive(Debug, Clone)]
pub struct NewNamedTimeSegment {
    pub name: String,
    // ranges is assumed to be in order
    pub ranges: Vec<Range<DateTime<Utc>>>,
    pub start: DateTime<Utc>,
    pub period: Duration,
    pub hue: u16,
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

impl PartialEq<NewNamedTimeSegment> for NamedTimeSegment {
    fn eq(&self, other: &NewNamedTimeSegment) -> bool {
        self.name == other.name
            && self.ranges == other.ranges
            && self.start == other.start
            && self.period == other.period
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
                start + Duration::hours(3 * 24 + 16)..start + Duration::hours(3 * 24 + 18),
                start + Duration::hours(3 * 24 + 19)..start + Duration::hours(3 * 24 + 21),
            ],
            start,
            period,
        };
        let inverse = UnnamedTimeSegment {
            ranges: vec![
                start..start + Duration::hours(24 + 10),
                start + Duration::hours(24 + 15)..start + Duration::hours(3 * 24 + 16),
                start + Duration::hours(3 * 24 + 18)..start + Duration::hours(3 * 24 + 19),
                start + Duration::hours(3 * 24 + 21)..start + period,
            ],
            start,
            period,
        };
        assert_eq!(segment.inverse(), inverse);
        assert_eq!(inverse.inverse(), segment);
    }

    #[test]
    fn generate_ranges_normal_cases() {
        let time0 = Utc::now();
        let time1 = time0 + Duration::days(10);
        let time2 = time0 + Duration::days(20);
        let time3 = time0 + Duration::days(30);

        // The segment starts at time1 and we'll check whether we can generate ranges in the past
        // and in the future using time0, time2 and time3.
        let segment = UnnamedTimeSegment {
            ranges: vec![
                time1 + Duration::hours(24 + 10)..time1 + Duration::hours(24 + 15),
                time1 + Duration::hours(3 * 24 + 16)..time1 + Duration::hours(3 * 24 + 18),
                time1 + Duration::hours(3 * 24 + 19)..time1 + Duration::hours(3 * 24 + 21),
            ],
            start: time1,
            period: Duration::weeks(1),
        };

        // Trivial cases: nothing to generate
        assert_eq!(segment.generate_ranges(time0, time0), vec![]);
        assert_eq!(segment.generate_ranges(time1, time0), vec![]);
        assert_eq!(segment.generate_ranges(time1, time1), vec![]);
        assert_eq!(segment.generate_ranges(time2, time0), vec![]);
        assert_eq!(segment.generate_ranges(time2, time1), vec![]);
        assert_eq!(segment.generate_ranges(time2, time2), vec![]);
        assert_eq!(segment.generate_ranges(time3, time0), vec![]);
        assert_eq!(segment.generate_ranges(time3, time1), vec![]);
        assert_eq!(segment.generate_ranges(time3, time2), vec![]);
        assert_eq!(segment.generate_ranges(time3, time3), vec![]);

        // Easy cases: the start of the generation is the same as the segment start
        assert_eq!(
            segment.generate_ranges(time1, time2),
            vec![
                time1 + Duration::hours(24 + 10)..time1 + Duration::hours(24 + 15),
                time1 + Duration::hours(3 * 24 + 16)..time1 + Duration::hours(3 * 24 + 18),
                time1 + Duration::hours(3 * 24 + 19)..time1 + Duration::hours(3 * 24 + 21),
                time1 + Duration::hours((7 + 1) * 24 + 10)
                    ..time1 + Duration::hours((7 + 1) * 24 + 15),
            ]
        );
        assert_eq!(
            segment.generate_ranges(time1, time3),
            vec![
                time1 + Duration::hours(24 + 10)..time1 + Duration::hours(24 + 15),
                time1 + Duration::hours(3 * 24 + 16)..time1 + Duration::hours(3 * 24 + 18),
                time1 + Duration::hours(3 * 24 + 19)..time1 + Duration::hours(3 * 24 + 21),
                time1 + Duration::hours((7 + 1) * 24 + 10)
                    ..time1 + Duration::hours((7 + 1) * 24 + 15),
                time1 + Duration::hours((7 + 3) * 24 + 16)
                    ..time1 + Duration::hours((7 + 3) * 24 + 18),
                time1 + Duration::hours((7 + 3) * 24 + 19)
                    ..time1 + Duration::hours((7 + 3) * 24 + 21),
                time1 + Duration::hours((14 + 1) * 24 + 10)
                    ..time1 + Duration::hours((14 + 1) * 24 + 15),
                time1 + Duration::hours((14 + 3) * 24 + 16)
                    ..time1 + Duration::hours((14 + 3) * 24 + 18),
                time1 + Duration::hours((14 + 3) * 24 + 19)
                    ..time1 + Duration::hours((14 + 3) * 24 + 21),
            ]
        );

        // Interesting cases: the start of the generation is before the start of the segment
        assert_eq!(
            segment.generate_ranges(time0, time1),
            vec![
                time0 + Duration::hours(4 * 24 + 10)..time0 + Duration::hours(4 * 24 + 15),
                time0 + Duration::hours(6 * 24 + 16)..time0 + Duration::hours(6 * 24 + 18),
                time0 + Duration::hours(6 * 24 + 19)..time0 + Duration::hours(6 * 24 + 21),
            ]
        );
        assert_eq!(
            segment.generate_ranges(time0, time2),
            vec![
                time0 + Duration::hours(4 * 24 + 10)..time0 + Duration::hours(4 * 24 + 15),
                time0 + Duration::hours(6 * 24 + 16)..time0 + Duration::hours(6 * 24 + 18),
                time0 + Duration::hours(6 * 24 + 19)..time0 + Duration::hours(6 * 24 + 21),
                time0 + Duration::hours((7 + 4) * 24 + 10)
                    ..time0 + Duration::hours((7 + 4) * 24 + 15),
                time0 + Duration::hours((7 + 6) * 24 + 16)
                    ..time0 + Duration::hours((7 + 6) * 24 + 18),
                time0 + Duration::hours((7 + 6) * 24 + 19)
                    ..time0 + Duration::hours((7 + 6) * 24 + 21),
                time0 + Duration::hours((14 + 4) * 24 + 10)
                    ..time0 + Duration::hours((14 + 4) * 24 + 15),
            ]
        );
        // Testing 0->3 as well seems a bit overkill

        // Interesting cases: the start of the generation is after the start of the segment
        assert_eq!(
            segment.generate_ranges(time2, time3),
            vec![
                time2 + Duration::hours(16)..time2 + Duration::hours(18),
                time2 + Duration::hours(19)..time2 + Duration::hours(21),
                time2 + Duration::hours(5 * 24 + 10)..time2 + Duration::hours(5 * 24 + 15),
                time2 + Duration::hours(7 * 24 + 16)..time2 + Duration::hours(7 * 24 + 18),
                time2 + Duration::hours(7 * 24 + 19)..time2 + Duration::hours(7 * 24 + 21),
            ]
        );

        // Tricky case: the start of the generation is inside of the segment
        let time4 = time1 + Duration::hours(24 + 12);
        assert_eq!(
            segment.generate_ranges(time4, time4 + Duration::days(10)),
            vec![
                time4..time4 + Duration::hours(3),
                time4 + Duration::hours(2 * 24 + 4)..time4 + Duration::hours(2 * 24 + 6),
                time4 + Duration::hours(2 * 24 + 7)..time4 + Duration::hours(2 * 24 + 9),
                time4 + Duration::hours(7 * 24 + -2)..time4 + Duration::hours(7 * 24 + 3),
                time4 + Duration::hours(9 * 24 + 4)..time4 + Duration::hours(9 * 24 + 6),
                time4 + Duration::hours(9 * 24 + 7)..time4 + Duration::hours(9 * 24 + 9),
            ]
        );

        // Tricky case: the end of the generation is inside of the segment
        assert_eq!(
            segment.generate_ranges(time1, time1 + Duration::hours(3 * 24 + 17)),
            vec![
                time1 + Duration::hours(24 + 10)..time1 + Duration::hours(24 + 15),
                time1 + Duration::hours(3 * 24 + 16)..time1 + Duration::hours(3 * 24 + 17),
            ]
        );
    }

    #[test]
    fn with_start() {
        let start = Utc::now();
        let period = Duration::weeks(1);
        let segment = UnnamedTimeSegment {
            ranges: vec![
                start + Duration::hours(24 + 10)..start + Duration::hours(24 + 15),
                start + Duration::hours(3 * 24 + 16)..start + Duration::hours(3 * 24 + 18),
                start + Duration::hours(3 * 24 + 19)..start + Duration::hours(3 * 24 + 21),
            ],
            start,
            period,
        };
        // If we shift it back a day, the ranges should stay the same, since they're still in the
        // same period.
        assert_eq!(
            segment.with_start(start - Duration::days(1)),
            UnnamedTimeSegment {
                ranges: vec![
                    start + Duration::hours(24 + 10)..start + Duration::hours(24 + 15),
                    start + Duration::hours(3 * 24 + 16)..start + Duration::hours(3 * 24 + 18),
                    start + Duration::hours(3 * 24 + 19)..start + Duration::hours(3 * 24 + 21),
                ],
                start: start - Duration::days(1),
                period,
            }
        );
        // If we shift it back a week, the ranges should shift a week, since they're the previous
        // period.
        assert_eq!(
            segment.with_start(start - Duration::weeks(1)),
            UnnamedTimeSegment {
                ranges: vec![
                    start + Duration::hours((-7 + 1) * 24 + 10)
                        ..start + Duration::hours((-7 + 1) * 24 + 15),
                    start + Duration::hours((-7 + 3) * 24 + 16)
                        ..start + Duration::hours((-7 + 3) * 24 + 18),
                    start + Duration::hours((-7 + 3) * 24 + 19)
                        ..start + Duration::hours((-7 + 3) * 24 + 21),
                ],
                start: start - Duration::weeks(1),
                period,
            }
        );
        // It gets a bit trickier here: if we shift backwards to a time between two ranges, the
        // ranges should reshuffle since we expect that the ranges are chronological.
        assert_eq!(
            segment.with_start(start - Duration::days(4)),
            UnnamedTimeSegment {
                ranges: vec![
                    start + Duration::hours((-7 + 3) * 24 + 16)
                        ..start + Duration::hours((-7 + 3) * 24 + 18),
                    start + Duration::hours((-7 + 3) * 24 + 19)
                        ..start + Duration::hours((-7 + 3) * 24 + 21),
                    start + Duration::hours(24 + 10)..start + Duration::hours(24 + 15),
                ],
                start: start - Duration::days(4),
                period,
            }
        );

        // We expect three similar cases if shift forwards
        assert_eq!(
            segment.with_start(start + Duration::days(1)),
            UnnamedTimeSegment {
                ranges: vec![
                    start + Duration::hours(24 + 10)..start + Duration::hours(24 + 15),
                    start + Duration::hours(3 * 24 + 16)..start + Duration::hours(3 * 24 + 18),
                    start + Duration::hours(3 * 24 + 19)..start + Duration::hours(3 * 24 + 21),
                ],
                start: start + Duration::days(1),
                period,
            }
        );
        assert_eq!(
            segment.with_start(start + Duration::weeks(1)),
            UnnamedTimeSegment {
                ranges: vec![
                    start + Duration::hours((7 + 1) * 24 + 10)
                        ..start + Duration::hours((7 + 1) * 24 + 15),
                    start + Duration::hours((7 + 3) * 24 + 16)
                        ..start + Duration::hours((7 + 3) * 24 + 18),
                    start + Duration::hours((7 + 3) * 24 + 19)
                        ..start + Duration::hours((7 + 3) * 24 + 21),
                ],
                start: start + Duration::weeks(1),
                period,
            }
        );
        assert_eq!(
            segment.with_start(start + Duration::days(2)),
            UnnamedTimeSegment {
                ranges: vec![
                    start + Duration::hours(3 * 24 + 16)..start + Duration::hours(3 * 24 + 18),
                    start + Duration::hours(3 * 24 + 19)..start + Duration::hours(3 * 24 + 21),
                    start + Duration::hours((7 + 1) * 24 + 10)
                        ..start + Duration::hours((7 + 1) * 24 + 15),
                ],
                start: start + Duration::days(2),
                period,
            }
        );

        // More tricky cases: if the new start is within segment, we expect it to be split up
        assert_eq!(
            segment.with_start(start + Duration::hours(24 + 12)),
            UnnamedTimeSegment {
                ranges: vec![
                    start + Duration::hours(24 + 12)..start + Duration::hours(24 + 15),
                    start + Duration::hours(3 * 24 + 16)..start + Duration::hours(3 * 24 + 18),
                    start + Duration::hours(3 * 24 + 19)..start + Duration::hours(3 * 24 + 21),
                    start + Duration::hours(8 * 24 + 10)..start + Duration::hours(8 * 24 + 12),
                ],
                start: start + Duration::hours(24 + 12),
                period,
            }
        );
    }
}
