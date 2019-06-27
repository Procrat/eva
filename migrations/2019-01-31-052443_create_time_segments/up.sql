CREATE TABLE time_segments (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  name TEXT NOT NULL,
  start INTEGER NOT NULL,
  period INTEGER NOT NULL
);

CREATE TABLE time_segment_ranges (
  segment_id INTEGER NOT NULL,
  start INTEGER NOT NULL,
  end INTEGER NOT NULL
);

-- Add a default time segment: daily from 9 to 5
INSERT INTO time_segments
VALUES (
  0,
  'Default',
  strftime('%s', 'now', 'weekday 1', 'start of day', 'utc', '9 hours'),
  24 * 60 * 60
);
INSERT INTO time_segment_ranges
VALUES (
  0,
  strftime('%s', 'now', 'weekday 1', 'start of day', 'utc', '9 hours'),
  strftime('%s', 'now', 'weekday 1', 'start of day', 'utc', '17 hours')
);

ALTER TABLE tasks
  ADD COLUMN time_segment_id NOT NULL DEFAULT 0;
