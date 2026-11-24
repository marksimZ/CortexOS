ALTER TABLE reminders ADD COLUMN news_query TEXT;
ALTER TABLE reminder_runs ADD COLUMN brief_query TEXT;
ALTER TABLE reminder_runs ADD COLUMN brief_status TEXT
    CHECK (brief_status IN ('pending', 'ready', 'empty', 'failed'));
ALTER TABLE reminder_runs ADD COLUMN brief_results TEXT;
ALTER TABLE reminder_runs ADD COLUMN brief_error TEXT;

CREATE INDEX reminder_runs_pending_briefs ON reminder_runs(scheduled_at, id)
    WHERE brief_status = 'pending';
