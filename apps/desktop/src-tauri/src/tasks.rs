use crate::models::AppResult;
use serde::Serialize;
use std::future::{Future, poll_fn};
use std::sync::{Arc, Mutex};
use std::task::Poll;
use tokio::sync::{OwnedSemaphorePermit, Semaphore, watch};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    Chat,
    Benchmark,
    HarnessValidation,
    HarnessWorkspace,
    HarnessSession,
    HarnessExperiment,
    RagIndex,
    LocalMaintenance,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSummary {
    pub id: String,
    pub workspace_id: String,
    pub kind: TaskKind,
    pub execution_run_id: Option<String>,
    #[serde(skip)]
    subject_id: Option<String>,
}
impl TaskSummary {
    fn new(id: &str, workspace: &str, kind: TaskKind) -> Self {
        Self {
            id: id.into(),
            workspace_id: workspace.into(),
            kind,
            execution_run_id: (kind == TaskKind::Chat).then(|| id.into()),
            subject_id: None,
        }
    }
}
struct Active {
    summary: TaskSummary,
    cancel: watch::Sender<bool>,
}
pub struct TaskSlot {
    active: Mutex<Option<Active>>,
    permit: Arc<Semaphore>,
    waiting: Mutex<Vec<Active>>,
}
impl Default for TaskSlot {
    fn default() -> Self {
        Self {
            active: Mutex::default(),
            permit: Arc::new(Semaphore::new(1)),
            waiting: Mutex::default(),
        }
    }
}
struct Waiting<'a> {
    slot: &'a TaskSlot,
    id: &'a str,
}
impl Drop for Waiting<'_> {
    fn drop(&mut self) {
        if let Ok(mut q) = self.slot.waiting.lock() {
            q.retain(|t| t.summary.id != self.id);
        }
    }
}
pub struct Lease {
    _permit: OwnedSemaphorePermit,
    slot: Arc<TaskSlot>,
    pub id: String,
    receiver: watch::Receiver<bool>,
}

impl TaskSlot {
    pub fn claim(self: &Arc<Self>, id: &str, workspace: &str, kind: TaskKind) -> AppResult<Lease> {
        let queue = self.waiting.lock().map_err(|_| "等待队列不可用。")?;
        if !queue.is_empty() {
            return Err(self.conflict());
        }
        let permit = self
            .permit
            .clone()
            .try_acquire_owned()
            .map_err(|_| self.conflict())?;
        let (cancel, receiver) = watch::channel(false);
        self.activate(
            TaskSummary::new(id, workspace, kind),
            permit,
            cancel,
            receiver,
        )
    }
    fn conflict(&self) -> String {
        let owner = match self.summary().ok().flatten().map(|s| s.kind) {
            Some(TaskKind::Chat) => "对话",
            Some(TaskKind::Benchmark) => "评测或项目任务",
            Some(TaskKind::HarnessValidation) => "项目任务验证",
            Some(TaskKind::HarnessWorkspace) => "项目任务副本",
            Some(TaskKind::HarnessSession) => "项目单题执行",
            Some(TaskKind::HarnessExperiment) => "项目组合实验",
            Some(TaskKind::RagIndex) => "索引构建",
            Some(TaskKind::LocalMaintenance) => "本机维护",
            None => "已排队的任务",
        };
        format!("已有任务正在进行，请先停止或等待完成。当前占用：{owner}。")
    }
    pub async fn wait_task(
        self: &Arc<Self>,
        id: &str,
        workspace: &str,
        kind: TaskKind,
        closing: &std::sync::atomic::AtomicBool,
        notify: impl Fn(&str),
    ) -> AppResult<Lease> {
        self.wait(TaskSummary::new(id, workspace, kind), closing, notify)
            .await
    }
    pub async fn wait_scoped(
        self: &Arc<Self>,
        id: &str,
        workspace: &str,
        kind: TaskKind,
        subject: &str,
        closing: &std::sync::atomic::AtomicBool,
    ) -> AppResult<Lease> {
        let mut request = TaskSummary::new(id, workspace, kind);
        request.subject_id = Some(subject.into());
        self.wait(request, closing, |_| {}).await
    }
    async fn wait(
        self: &Arc<Self>,
        request: TaskSummary,
        closing: &std::sync::atomic::AtomicBool,
        notify: impl Fn(&str),
    ) -> AppResult<Lease> {
        let id = request.id.clone();
        let kind = request.kind;
        let duplicate = |summary: &TaskSummary| {
            summary.id == id
                || (request.subject_id.is_some()
                    && summary.kind == request.kind
                    && summary.workspace_id == request.workspace_id
                    && summary.subject_id == request.subject_id)
        };
        let (cancel, mut receiver) = watch::channel(false);
        let acquisition = self.permit.clone().acquire_owned();
        tokio::pin!(acquisition);
        let (first_poll, _waiting) = poll_fn(|cx| {
            Poll::Ready((|| -> AppResult<_> {
                let mut q = self.waiting.lock().map_err(|_| "等待队列不可用。")?;
                if q.iter().any(|a| duplicate(&a.summary))
                    || self.summary()?.as_ref().is_some_and(duplicate)
                {
                    return Err("此任务已提交，请查看当前进度。".into());
                }
                if closing.load(std::sync::atomic::Ordering::SeqCst) {
                    return Err("应用正在关闭。".into());
                }
                q.push(Active {
                    summary: request.clone(),
                    cancel: cancel.clone(),
                });
                // Register semaphore waiters in the same order as the visible queue.
                Ok((
                    acquisition.as_mut().poll(cx),
                    Waiting {
                        slot: self,
                        id: &id,
                    },
                ))
            })())
        })
        .await?;
        let permit = match first_poll {
            Poll::Ready(result) => result.map_err(|_| "队列已关闭。")?,
            Poll::Pending => {
                notify(&format!("正在等待。{}", self.conflict()));
                tokio::select! { biased;
                    _ = receiver.wait_for(|v| *v) => return Err("已取消等待。".into()),
                    p = &mut acquisition => p.map_err(|_| "队列已关闭。")?,
                }
            }
        };
        let mut q = self.waiting.lock().map_err(|_| "等待队列不可用。")?;
        if *receiver.borrow() || closing.load(std::sync::atomic::Ordering::SeqCst) {
            return Err("已取消等待。".into());
        }
        let lease = self.activate(request, permit, cancel, receiver)?;
        q.retain(|t| t.summary.id != id);
        drop(q);
        notify(match kind {
            TaskKind::RagIndex => "正在构建向量索引…",
            TaskKind::HarnessValidation => "正在启动验证…",
            _ => "正在启动评测…",
        });
        Ok(lease)
    }
    fn activate(
        self: &Arc<Self>,
        summary: TaskSummary,
        permit: OwnedSemaphorePermit,
        cancel: watch::Sender<bool>,
        receiver: watch::Receiver<bool>,
    ) -> AppResult<Lease> {
        let mut active = self.active.lock().map_err(|_| "任务状态不可用。")?;
        let id = summary.id.clone();
        *active = Some(Active { summary, cancel });
        Ok(Lease {
            _permit: permit,
            slot: self.clone(),
            id,
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
    pub fn scoped_request(
        &self,
        workspace: &str,
        kind: TaskKind,
        subject: &str,
    ) -> AppResult<Option<(String, bool)>> {
        let q = self.waiting.lock().map_err(|_| "等待队列不可用。")?;
        let matches = |a: &TaskSummary| {
            a.workspace_id == workspace
                && a.kind == kind
                && a.subject_id.as_deref() == Some(subject)
        };
        if let Some(a) = q.iter().find(|a| matches(&a.summary)) {
            return Ok(Some((a.summary.id.clone(), true)));
        }
        Ok(self.summary()?.filter(matches).map(|a| (a.id, false)))
    }
    pub fn cancel_waiting(&self, id: &str, workspace: &str, kind: TaskKind) -> AppResult<()> {
        self.cancel_waiting_for(id, workspace, kind, |_| true)
    }
    pub(crate) fn cancel_waiting_for(
        &self, id: &str, workspace: &str, kind: TaskKind,
        subject_matches: impl Fn(Option<&str>) -> bool,
    ) -> AppResult<()> {
        let q = self.waiting.lock().map_err(|_| "等待队列不可用。")?;
        let task = q
            .iter()
            .find(|t| {
                t.summary.id == id && t.summary.workspace_id == workspace && t.summary.kind == kind
                    && subject_matches(t.summary.subject_id.as_deref())
            })
            .ok_or("此任务已不在等待队列，请查看当前进度。")?;
        task.cancel.send_replace(true);
        Ok(())
    }
    pub fn is_waiting(&self, id: &str, workspace: &str, kind: TaskKind) -> AppResult<bool> {
        Ok(self
            .waiting
            .lock()
            .map_err(|_| "等待队列不可用。")?
            .iter()
            .any(|a| {
                a.summary.id == id && a.summary.workspace_id == workspace && a.summary.kind == kind
            }))
    }
    pub fn has_active(&self) -> bool {
        self.permit.available_permits() == 0
            || self.waiting.lock().map(|q| !q.is_empty()).unwrap_or(true)
    }
    pub fn cancel(&self, id: &str, workspace: &str, kind: TaskKind) -> AppResult<()> {
        let q = self.waiting.lock().map_err(|_| "等待队列不可用。")?;
        if let Some(task) = q.iter().find(|t| {
            t.summary.id == id && t.summary.workspace_id == workspace && t.summary.kind == kind
        }) {
            task.cancel.send_replace(true);
            return Ok(());
        }
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
        if let Ok(q) = self.waiting.lock() {
            for task in q.iter() {
                task.cancel.send_replace(true);
            }
        }
        if let Ok(active) = self.active.lock() {
            if let Some(a) = active.as_ref() {
                let _ = a.cancel.send(true);
            }
        }
    }
    pub fn cancel_kind(&self, workspace: &str, kind: TaskKind) -> AppResult<()> {
        let active = self
            .active
            .lock()
            .map_err(|_| "任务状态不可用，请重启应用。")?;
        match active.as_ref() {
            Some(task) if task.summary.workspace_id == workspace && task.summary.kind == kind => {
                task.cancel.send(true).map_err(|_| "任务已经结束。".into())
            }
            _ => Err("当前工作区没有正在构建的索引。".into()),
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

#[cfg(test)]
mod scheduler_tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    #[tokio::test]
    async fn queued_indexes_cancel_by_owner_and_receive_the_slot_in_order() {
        let slot = Arc::new(TaskSlot::default());
        let closing = AtomicBool::new(false);
        let active = slot.claim("chat", "one", TaskKind::Chat).unwrap();
        let first = slot.wait_task("first", "one", TaskKind::RagIndex, &closing, |_| {});
        let stopped = slot.wait_task("stop", "two", TaskKind::Benchmark, &closing, |_| {});
        let last = slot.wait_task("last", "two", TaskKind::Benchmark, &closing, |_| {});
        tokio::pin!(first, stopped, last);
        tokio::select! { biased;
            _ = &mut first => panic!("occupied"),
            _ = &mut stopped => panic!("occupied"),
            _ = &mut last => panic!("occupied"),
            _ = std::future::ready(()) => {}
        }
        assert!(slot.cancel("stop", "one", TaskKind::Benchmark).is_err());
        assert!(slot.is_waiting("stop", "two", TaskKind::Benchmark).unwrap());
        assert!(slot.cancel("stop", "two", TaskKind::RagIndex).is_err());
        slot.cancel_waiting("stop", "two", TaskKind::Benchmark)
            .unwrap();
        assert!(stopped.await.is_err());
        assert!(!slot.is_waiting("stop", "two", TaskKind::Benchmark).unwrap());
        assert!(!*active.receiver().borrow());
        drop(active);
        assert!(slot.claim("new", "one", TaskKind::Benchmark).is_err());
        let first = first.await.unwrap();
        assert_eq!(first.id, "first");
        assert!(
            slot.cancel_waiting("first", "one", TaskKind::RagIndex)
                .is_err()
        );
        drop(first);
        assert_eq!(last.await.unwrap().id, "last");
        assert!(!slot.has_active());
    }
}
