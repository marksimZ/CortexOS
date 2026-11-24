use crate::{
    benchmark::dataset::require_workspace,
    commands::{AppState, blocking},
    db::{Database, error},
    models::{AppResult, new_id},
    web_search::{WebResult, WebSearch, public_news, validate_query},
};
use chrono::{DateTime, Datelike, Local, NaiveDate, NaiveTime, TimeZone, Utc};
use rusqlite::{OptionalExtension, Row, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager, State};

const LATE_SECONDS: i64 = 5 * 60;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReminderInput {
    title: String,
    kind: String,
    date: String,
    time: String,
    weekday: u32,
    news_query: Option<String>,
}

struct Calendar<'a> {
    kind: &'a str,
    date: NaiveDate,
    time: NaiveTime,
    weekday: u32,
}

impl ReminderInput {
    fn calendar(&self) -> AppResult<Calendar<'_>> {
        let date = NaiveDate::parse_from_str(&self.date, "%Y-%m-%d")
            .ok()
            .filter(|date| (1..=9999).contains(&date.year()) && date.to_string() == self.date)
            .ok_or("提醒日期无效。")?;
        let time = NaiveTime::parse_from_str(&self.time, "%H:%M")
            .ok()
            .filter(|time| time.format("%H:%M").to_string() == self.time)
            .ok_or("提醒时间无效。")?;
        if !matches!(self.kind.as_str(), "once" | "daily" | "weekdays" | "weekly")
            || !(1..=7).contains(&self.weekday)
        {
            return Err("提醒重复规则无效。".into());
        }
        Ok(Calendar {
            kind: &self.kind,
            date,
            time,
            weekday: self.weekday,
        })
    }
}

impl Calendar<'_> {
    fn applies(&self, date: NaiveDate) -> bool {
        let weekday = date.weekday().number_from_monday();
        date >= self.date
            && match self.kind {
                "once" => date == self.date,
                "weekdays" => weekday <= 5,
                "weekly" => weekday == self.weekday,
                _ => true,
            }
    }

    fn at(&self, date: NaiveDate) -> Option<DateTime<Local>> {
        Local
            .from_local_datetime(&date.and_time(self.time))
            .earliest()
    }

    fn future_date(&self, now: DateTime<Local>) -> AppResult<NaiveDate> {
        if self.kind == "once" {
            return self
                .at(self.date)
                .filter(|time| *time > now)
                .map(|_| self.date)
                .ok_or_else(|| "单次提醒须使用未来的有效本地时间。".into());
        }
        let mut date = self.date.max(now.date_naive());
        loop {
            if self.applies(date) && self.at(date).is_some_and(|time| time > now) {
                return Ok(date);
            }
            date = date.succ_opt().ok_or("无法计算下一次提醒。")?;
        }
    }

    fn latest_due(&self, first: NaiveDate, now: DateTime<Local>) -> Option<DateTime<Local>> {
        if self.kind == "once" {
            return self.at(first).filter(|time| *time <= now);
        }
        let mut date = now.date_naive();
        while date >= first {
            if self.applies(date)
                && let Some(time) = self.at(date).filter(|time| *time <= now)
            {
                return Some(time);
            }
            date = date.pred_opt()?;
        }
        None
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Reminder {
    id: String,
    #[serde(flatten)]
    input: ReminderInput,
    state: String,
    next_at: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReminderRun {
    id: String,
    title: String,
    scheduled_at: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    brief: Option<NewsBrief>,
}

#[derive(Serialize)]
struct NewsBrief {
    status: String,
    query: String,
    results: Vec<WebResult>,
    error: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct ReminderList {
    reminders: Vec<Reminder>,
    runs: Vec<ReminderRun>,
}

fn read_input(row: &Row<'_>, start: usize) -> rusqlite::Result<ReminderInput> {
    Ok(ReminderInput {
        title: row.get(start)?,
        kind: row.get(start + 1)?,
        date: row.get(start + 2)?,
        time: row.get(start + 3)?,
        weekday: row.get(start + 4)?,
        news_query: row.get(start + 5)?,
    })
}

fn parse_date(date: &str) -> AppResult<NaiveDate> {
    date.parse().map_err(|_| "本地提醒日期无效。".into())
}

#[tauri::command]
pub(crate) async fn list_reminders(
    state: State<'_, AppState>,
    workspace_id: String,
) -> AppResult<ReminderList> {
    blocking(state.database.clone()?, move |db| {
        let connection = db.connection()?;
        require_workspace(&connection, &workspace_id)?;
        let mut query = connection
            .prepare(
                "SELECT id,title,kind,date,time,weekday,news_query,state,next_date FROM reminders
             WHERE workspace_id=?1 ORDER BY state!='active',next_date,time,title,id",
            )
            .map_err(error)?;
        let rows = query
            .query_map([&workspace_id], |row| {
                Ok((
                    row.get(0)?,
                    read_input(row, 1)?,
                    row.get(7)?,
                    row.get::<_, Option<String>>(8)?,
                ))
            })
            .map_err(error)?;
        let reminders = rows
            .map(|row| {
                let (id, input, state, next_date) = row.map_err(error)?;
                let next_at = match next_date {
                    Some(date) => input
                        .calendar()?
                        .at(parse_date(&date)?)
                        .map(|time| time.to_rfc3339()),
                    None => None,
                };
                Ok(Reminder {
                    id,
                    input,
                    state,
                    next_at,
                })
            })
            .collect::<AppResult<_>>()?;
        let mut query = connection
            .prepare(
                "SELECT id,title,scheduled_at,status,brief_query,brief_status,brief_results,brief_error
                 FROM reminder_runs
             WHERE workspace_id=?1 ORDER BY scheduled_at DESC,id DESC LIMIT 100",
            )
            .map_err(error)?;
        let runs = query
            .query_map([&workspace_id], |row| {
                Ok((
                    row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?,
                    row.get::<_, Option<String>>(4)?, row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?, row.get(7)?,
                ))
            })
            .map_err(error)?
            .map(|row| {
                let (id, title, timestamp, status, query, brief_status, results, brief_error) =
                    row.map_err(error)?;
                let scheduled_at = DateTime::<Utc>::from_timestamp(timestamp, 0)
                    .ok_or("本地提醒记录时间无效。")?
                    .to_rfc3339();
                let brief = if let Some(query) = query {
                    Some(NewsBrief {
                        status: brief_status.ok_or("简报记录缺少处理状态。")?,
                        query,
                        results: serde_json::from_str(results.as_deref().unwrap_or("[]"))
                            .map_err(crate::db::json_error)?,
                        error: brief_error,
                    })
                } else {
                    None
                };
                Ok(ReminderRun {
                    id,
                    title,
                    scheduled_at,
                    status,
                    brief,
                })
            })
            .collect::<AppResult<_>>()?;
        Ok(ReminderList { reminders, runs })
    })
    .await
}

#[tauri::command]
pub(crate) async fn save_reminder(
    state: State<'_, AppState>,
    workspace_id: String,
    id: Option<String>,
    input: ReminderInput,
) -> AppResult<()> {
    let title = input.title.trim();
    if title.is_empty() || title.chars().count() > 120 {
        return Err("提醒标题须包含 1 至 120 个字符。".into());
    }
    let news_query = input
        .news_query
        .as_deref()
        .map(validate_query)
        .transpose()?
        .map(str::to_owned);
    blocking(state.database.clone()?, move |db| {
        let mut conn = db.connection()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(error)?;
        require_workspace(&tx, &workspace_id)?;
        let next_date = input.calendar()?.future_date(Local::now())?.to_string();
        let title = input.title.trim();
        if let Some(id) = id {
            let updated = tx.execute(
                "UPDATE reminders SET title=?3,kind=?4,date=?5,time=?6,weekday=?7,news_query=?9,
                   state=CASE WHEN state='paused' THEN 'paused' ELSE 'active' END,
                   next_date=CASE WHEN state='paused' THEN NULL ELSE ?8 END
                 WHERE workspace_id=?1 AND id=?2 AND state IN ('active', 'paused')",
                params![workspace_id, id, title, input.kind, input.date, input.time, input.weekday, next_date, news_query],
            ).map_err(error)?;
            if updated == 0 {
                return Err("此提醒不存在或已结束。".into());
            }
        } else {
            tx.execute(
                "INSERT INTO reminders(workspace_id,id,title,kind,date,time,weekday,state,next_date,news_query)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,'active',?8,?9)",
                params![workspace_id, new_id(), title, input.kind, input.date, input.time, input.weekday, next_date, news_query],
            ).map_err(error)?;
        }
        tx.commit().map_err(error)
    }).await
}

#[tauri::command]
pub(crate) async fn set_reminder_state(
    app_state: State<'_, AppState>,
    workspace_id: String,
    id: String,
    state: String,
) -> AppResult<()> {
    if !matches!(state.as_str(), "active" | "paused" | "cancelled") {
        return Err("提醒状态无效。".into());
    }
    blocking(app_state.database.clone()?, move |db| {
        let mut conn = db.connection()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(error)?;
        let (input, previous): (ReminderInput, String) = tx.query_row(
            "SELECT title,kind,date,time,weekday,news_query,state FROM reminders WHERE workspace_id=?1 AND id=?2",
            params![workspace_id, id], |row| Ok((read_input(row, 0)?, row.get(6)?)),
        ).optional().map_err(error)?.ok_or("此提醒不存在。")?;
        if previous == state {
            return Ok(());
        }
        if matches!(previous.as_str(), "cancelled" | "completed") {
            return Err("此提醒已结束。".into());
        }
        let next_date = if state == "active" {
            Some(input.calendar()?.future_date(Local::now())?.to_string())
        } else {
            None
        };
        tx.execute(
            "UPDATE reminders SET state=?3,next_date=?4 WHERE workspace_id=?1 AND id=?2",
            params![workspace_id, id, state, next_date],
        ).map_err(error)?;
        tx.commit().map_err(error)
    }).await
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Delivery {
    workspace_id: String,
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    brief_status: Option<&'static str>,
}

fn due(db: &Database) -> AppResult<Vec<Delivery>> {
    let mut conn = db.connection()?;
    let tx = conn.transaction().map_err(error)?;
    let now = Local::now();
    let pending = {
        let mut query = tx
            .prepare(
                "SELECT id,workspace_id,title,kind,date,time,weekday,news_query,next_date FROM reminders
             WHERE state='active' AND next_date<=?1 ORDER BY next_date,time,id",
            )
            .map_err(error)?;
        query
            .query_map([now.date_naive().to_string()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    read_input(row, 2)?,
                    row.get::<_, String>(8)?,
                ))
            })
            .map_err(error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(error)?
    };
    let mut deliveries = Vec::new();
    for (id, workspace_id, input, next_date) in pending {
        let date = parse_date(&next_date)?;
        let calendar = input.calendar()?;
        let first = calendar.at(date);
        if first.is_some_and(|time| time > now) {
            continue;
        }
        let latest = calendar.latest_due(date, now);
        let record = |time: DateTime<Local>, status| {
            let (brief_status, brief_error) = match (&input.news_query, status) {
                (Some(_), "missed") => (
                    Some("failed"),
                    Some("已超过提醒有效时间，跳过本次新闻查询。"),
                ),
                (Some(_), _) => (Some("pending"), None),
                (None, _) => (None, None),
            };
            tx.execute(
                "INSERT INTO reminder_runs(id,workspace_id,reminder_id,title,scheduled_at,status,brief_query,brief_status,brief_error)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)
                 ON CONFLICT(reminder_id,scheduled_at) DO NOTHING",
                params![new_id(), workspace_id, id, input.title, time.timestamp(), status, input.news_query, brief_status, brief_error],
            ).map_err(error)
        };
        let current =
            latest.filter(|time| now.signed_duration_since(*time).num_seconds() <= LATE_SECONDS);
        if let Some(time) = first.or(latest).filter(|time| Some(*time) != current) {
            record(time, "missed")?;
        }
        let delivered = match current {
            Some(time) => record(time, "delivered")? > 0,
            None => false,
        };
        let next = if calendar.kind == "once" {
            None
        } else {
            Some(calendar.future_date(now)?.to_string())
        };
        tx.execute(
            "UPDATE reminders SET next_date=?2,state=CASE WHEN ?2 IS NULL THEN 'completed' ELSE 'active' END
             WHERE id=?1",
            params![id, next],
        ).map_err(error)?;
        if delivered && input.news_query.is_none() {
            deliveries.push(Delivery {
                workspace_id,
                title: input.title,
                brief_status: None,
            });
        }
    }
    tx.commit().map_err(error)?;
    Ok(deliveries)
}

struct BriefJob {
    id: String,
    workspace_id: String,
    title: String,
    query: String,
}

fn next_brief(db: &Database) -> AppResult<Option<BriefJob>> {
    db.connection()?
        .query_row(
            "SELECT id,workspace_id,title,brief_query FROM reminder_runs
         WHERE brief_status='pending' ORDER BY scheduled_at,id LIMIT 1",
            [],
            |row| {
                Ok(BriefJob {
                    id: row.get(0)?,
                    workspace_id: row.get(1)?,
                    title: row.get(2)?,
                    query: row.get(3)?,
                })
            },
        )
        .optional()
        .map_err(error)
}

fn interrupted_briefs(db: &Database) -> AppResult<()> {
    db.connection()?
        .execute(
            "UPDATE reminder_runs SET brief_status='failed',
            brief_error='应用退出时新闻查询尚未完成。可手动查询，或等待下次计划。'
         WHERE brief_status='pending'",
            [],
        )
        .map_err(error)?;
    Ok(())
}

async fn prepare_briefs(app: AppHandle, database: Arc<Database>, closing: Arc<AtomicBool>) {
    // ponytail: a single news worker bounds outbound concurrency; ordinary reminder delivery runs separately.
    while !closing.load(Ordering::Relaxed) {
        let job = match blocking(database.clone(), next_brief).await {
            Ok(Some(job)) => job,
            other => {
                if let Err(message) = other {
                    eprintln!("reminder briefing queue failed: {message}");
                }
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };
        if closing.load(Ordering::Relaxed) {
            return;
        }
        let response = public_news(&app.state::<WebSearch>(), &job.query)
            .await
            .and_then(|results| {
                let status = if results.is_empty() { "empty" } else { "ready" };
                let payload = serde_json::to_string(&results).map_err(crate::db::json_error)?;
                Ok((status, payload))
            });
        let (status, payload, failure) = match response {
            Ok((status, payload)) => (status, Some(payload), None),
            Err(failure) => ("failed", None, Some(failure)),
        };
        while !closing.load(Ordering::Relaxed) {
            let id = job.id.clone();
            let payload = payload.clone();
            let failure = failure.clone();
            let saved =
                blocking(database.clone(), move |db| {
                    db.connection()?.execute(
                    "UPDATE reminder_runs SET brief_status=?2,brief_results=?3,brief_error=?4
                     WHERE id=?1 AND brief_status='pending'",
                    params![id, status, payload, failure],
                ).map_err(error)
                })
                .await;
            match saved {
                Ok(updated) => {
                    if updated > 0 {
                        emit_delivery(
                            &app,
                            Delivery {
                                workspace_id: job.workspace_id,
                                title: job.title,
                                brief_status: Some(status),
                            },
                        );
                    }
                    break;
                }
                Err(message) => {
                    eprintln!("reminder briefing write failed: {message}");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }
}

fn emit_delivery(app: &AppHandle, delivery: Delivery) {
    if let Err(error) = app.emit("reminder-due", delivery) {
        eprintln!("reminder notification failed: {error}");
    }
}

pub(crate) fn start_scheduler(app: &AppHandle, state: &AppState) {
    // The database handle keeps Windows' exclusive data-directory lock through recovery and delivery.
    let Ok(database) = state.database.clone() else {
        return;
    };
    let closing = state.closing.clone();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            if closing.load(Ordering::Relaxed) {
                return;
            }
            match blocking(database.clone(), interrupted_briefs).await {
                Ok(()) => break,
                Err(message) => {
                    eprintln!("reminder briefing recovery failed: {message}");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
        tauri::async_runtime::spawn(prepare_briefs(
            app.clone(),
            database.clone(),
            closing.clone(),
        ));
        while !closing.load(Ordering::Relaxed) {
            tokio::time::sleep(Duration::from_secs(5)).await;
            if closing.load(Ordering::Relaxed) {
                break;
            }
            match blocking(database.clone(), due).await {
                Ok(deliveries) => {
                    for delivery in deliveries {
                        emit_delivery(&app, delivery);
                    }
                }
                Err(error) => eprintln!("reminder scheduler failed: {error}"),
            }
        }
    });
}
