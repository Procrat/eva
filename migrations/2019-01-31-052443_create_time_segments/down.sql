ALTER TABLE tasks RENAME TO oldTasks;
CREATE TABLE tasks (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  content TEXT NOT NULL,
  deadline TEXT NOT NULL,
  duration INTEGER NOT NULL,
  importance INTEGER NOT NULL
);
INSERT INTO tasks (id, content, deadline, duration, importance)
  SELECT id, content, deadline, duration, importance FROM oldTasks;
DROP TABLE oldTasks;

DROP TABLE time_segment_ranges;

DROP TABLE time_segments;
