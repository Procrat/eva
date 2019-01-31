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

-- Add the default time segment that covers everything
INSERT INTO time_segments
VALUES (0, 'Default', 0, 7 * 42 * 60 * 60);
INSERT INTO time_segment_ranges
VALUES (0, 0, 7 * 42 * 60 * 60);

ALTER TABLE tasks
  ADD COLUMN time_segment_id NOT NULL DEFAULT 0;
