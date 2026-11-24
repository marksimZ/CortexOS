CREATE TABLE reminders (
    id TEXT PRIMARY KEY NOT NULL,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('once', 'daily', 'weekdays', 'weekly')),
    date TEXT NOT NULL,
    time TEXT NOT NULL,
    weekday INTEGER NOT NULL CHECK (weekday BETWEEN 1 AND 7),
    state TEXT NOT NULL CHECK (state IN ('active', 'paused', 'cancelled', 'completed')),
    next_date TEXT,
    CHECK ((state = 'active') = (next_date IS NOT NULL)),
    UNIQUE (workspace_id, id)
);

CREATE INDEX reminders_schedule ON reminders(next_date) WHERE state = 'active';

CREATE TABLE reminder_runs (
    id TEXT PRIMARY KEY NOT NULL,
    workspace_id TEXT NOT NULL,
    reminder_id TEXT NOT NULL,
    title TEXT NOT NULL,
    scheduled_at INTEGER NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('delivered', 'missed')),
    UNIQUE (reminder_id, scheduled_at),
    FOREIGN KEY (workspace_id, reminder_id) REFERENCES reminders(workspace_id, id) ON DELETE CASCADE
);

CREATE INDEX reminder_runs_recent ON reminder_runs(workspace_id, scheduled_at DESC, id DESC);
