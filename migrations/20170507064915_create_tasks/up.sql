CREATE TABLE tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    content TEXT NOT NULL,
    deadline TEXT NOT NULL,
    duration INTEGER NOT NULL,
    importance INTEGER NOT NULL
)
