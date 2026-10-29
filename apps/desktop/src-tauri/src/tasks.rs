use crate::models::AppResult;
use serde::Serialize;
use std::sync::{Arc, Mutex};
use tokio::sync::watch;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    Chat,
    Benchmark,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSummary {
    pub id: String,
    pub workspace_id: String,
    pub kind: TaskKind,
    pub execution_run_id: Option<String>,
}
struct Active {
    summary: TaskSummary,
    cancel: watch::Sender<bool>,
}
#[derive(Default)]
pub struct TaskSlot {
    active: Mutex<Option<Active>>,
}
pub struct Lease {
    slot: Arc<TaskSlot>,
    pub id: String,
    receiver: watch::Receiver<bool>,
}

impl TaskSlot {
    pub fn claim(self: &Arc<Self>, id: &str, workspace: &str, kind: TaskKind) -> AppResult<Lease> {
        let mut active = self
            .active
            .lock()
            .map_err(|_| "任务状态不可用，请重启应用。")?;
        if active.is_some() {
            return Err("已有聊天或评测正在进行，请先停止或等待完成。".into());
        }
        let (cancel, receiver) = watch::channel(false);
        *active = Some(Active {
            summary: TaskSummary {
                id: id.into(),
                workspace_id: workspace.into(),
                kind,
                execution_run_id: if kind == TaskKind::Chat {
                    Some(id.into())
                } else {
                    None
                },
            },
            cancel,
        });
        Ok(Lease {
            slot: self.clone(),
            id: id.into(),
            receiver,
        })
    }
    pub fn summary(&self) -> AppResult<Option<TaskSummary>> {
        Ok(self
            .active
            .lock()
            .map_err(|_| "任务状态不可用。")?
            .as_ref()
            .map(|a| a.summary.clone()))
    }
    pub fn has_active(&self) -> bool {
        self.active.lock().map(|a| a.is_some()).unwrap_or(true)
    }
    pub fn cancel(&self, id: &str, workspace: &str, kind: TaskKind) -> AppResult<()> {
        let active = self.active.lock().map_err(|_| "任务状态不可用。")?;
        if let Some(a) = active.as_ref() {
            if a.summary.id != id || a.summary.workspace_id != workspace || a.summary.kind != kind {
                return Err("所选任务不属于当前工作区，或已有其他任务。".into());
            }
            a.cancel.send(true).map_err(|_| "任务已经结束。")?;
        }
        Ok(())
    }
    pub fn cancel_active(&self) {
        if let Ok(active) = self.active.lock() {
            if let Some(a) = active.as_ref() {
                let _ = a.cancel.send(true);
            }
        }
    }
}
impl Lease {
    pub fn receiver(&self) -> watch::Receiver<bool> {
        self.receiver.clone()
    }
    pub fn set_execution(&self, id: &str) -> AppResult<()> {
        let mut active = self.slot.active.lock().map_err(|_| "任务状态不可用。")?;
        let a = active
            .as_mut()
            .filter(|a| a.summary.id == self.id)
            .ok_or("当前任务已失效。")?;
        a.summary.execution_run_id = Some(id.into());
        Ok(())
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        if let Ok(mut active) = self.slot.active.lock() {
            if active.as_ref().is_some_and(|a| a.summary.id == self.id) {
                *active = None;
            }
        }
    }
}
