use crate::{db::Database, models::*};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionView {
    pub revision: i64,
    pub camera: [f64; 3],
    pub target: [f64; 3],
}
#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveExecutionView {
    pub workspace_id: String,
    pub run_id: String,
    pub expected_revision: i64,
    pub camera: [f64; 3],
    pub target: [f64; 3],
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionGraph {
    pub detail: RunDetail,
    pub output: Message,
    pub view: ExecutionView,
}
fn error(_: rusqlite::Error) -> String {
    "执行视图读取或保存失败，请重试。".into()
}
impl Database {
    pub(crate) fn personalized_execution(&self, workspace: &str, run: &str) -> AppResult<bool> {
        self.connection()?.query_row("SELECT EXISTS(SELECT 1 FROM benchmark_attempts a JOIN benchmark_case_runs c ON c.id=a.case_run_id JOIN benchmark_runs b ON b.id=c.run_id WHERE a.execution_run_id=?1 AND a.workspace_id=?2 AND b.workspace_id=?2 AND b.dataset_id LIKE 'personal-%')",params![run,workspace],|r|r.get(0)).map_err(error)
    }
    pub fn execution_graph(&self, workspace: &str, run: &str) -> AppResult<ExecutionGraph> {
        self.recorded_execution_graph(workspace, run, ExecutionPurpose::Chat)
    }
    pub(crate) fn execution_purpose(
        &self,
        workspace: &str,
        run: &str,
    ) -> AppResult<ExecutionPurpose> {
        let raw:String=self.connection()?.query_row("SELECT c.purpose FROM execution_runs r JOIN conversations c ON c.id=r.conversation_id AND c.workspace_id=r.workspace_id WHERE r.workspace_id=?1 AND r.id=?2",params![workspace,run],|r|r.get(0)).map_err(error)?;
        match raw.as_str() {
            "chat" => Ok(ExecutionPurpose::Chat),
            "benchmark" => Ok(ExecutionPurpose::Benchmark),
            _ => Err("运行用途无效。".into()),
        }
    }
    pub(crate) fn recorded_execution_graph(
        &self,
        workspace: &str,
        run: &str,
        purpose: ExecutionPurpose,
    ) -> AppResult<ExecutionGraph> {
        let mut detail = self.recorded_run_detail(workspace, run, purpose)?;
        let output = if matches!(purpose, ExecutionPurpose::Chat) {
            self.messages(workspace, &detail.run.conversation_id)?
                .into_iter()
                .find(|m| m.id == detail.run.assistant_message_id)
                .ok_or("Output missing")?
        } else {
            self.connection()?.query_row("SELECT id,workspace_id,conversation_id,role,content,status,model_run_id,created_at FROM messages WHERE workspace_id=?1 AND id=?2",params![workspace,detail.run.assistant_message_id],|r|Ok(Message{id:r.get(0)?,workspace_id:r.get(1)?,conversation_id:r.get(2)?,role:r.get(3)?,content:r.get(4)?,status:r.get(5)?,model_run_id:r.get(6)?,model_id:Some(detail.run.model_id.clone()),created_at:r.get(7)?,finish_reason:detail.events.iter().rev().find(|e|e.kind=="response_completed").and_then(|e|e.payload["metrics"]["finishReason"].as_str()).map(str::to_string),citations:None})).map_err(error)?
        };
        if detail.run.status == "running" && output.status != "streaming" {
            detail = self.recorded_run_detail(workspace, run, purpose)?;
        }
        let connection = self.connection()?;
        let saved:Option<(i64,String,String)>=connection.query_row("SELECT revision,camera_json,target_json FROM execution_layouts WHERE workspace_id=?1 AND run_id=?2",params![workspace,run],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(error)?;
        let view = match saved {
            Some((revision, camera, target)) => ExecutionView {
                revision,
                camera: serde_json::from_str(&camera).map_err(|_| "视角格式无效。")?,
                target: serde_json::from_str(&target).map_err(|_| "视角格式无效。")?,
            },
            None => ExecutionView {
                revision: 0,
                camera: [12., 10., 28.],
                target: [12., 0., 0.],
            },
        };
        Ok(ExecutionGraph {
            detail,
            output,
            view,
        })
    }
    pub fn save_execution_view(&self, input: SaveExecutionView) -> AppResult<i64> {
        self.save_recorded_execution_view(input, ExecutionPurpose::Chat)
    }
    pub(crate) fn save_recorded_execution_view(
        &self,
        input: SaveExecutionView,
        purpose: ExecutionPurpose,
    ) -> AppResult<i64> {
        if input.expected_revision < 0
            || input
                .camera
                .iter()
                .chain(input.target.iter())
                .any(|v| !v.is_finite() || v.abs() > 100000.)
            || input
                .camera
                .iter()
                .zip(input.target)
                .map(|(a, b)| (a - b).powi(2))
                .sum::<f64>()
                < 0.01
        {
            return Err("视角参数无效。".into());
        }
        let mut connection = self.connection()?;
        let tx = connection.transaction().map_err(error)?;
        let exists: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM execution_runs r JOIN conversations c ON c.id=r.conversation_id AND c.workspace_id=r.workspace_id WHERE r.workspace_id=?1 AND r.id=?2 AND c.purpose=?3)",
                params![input.workspace_id, input.run_id,purpose.as_str()],
                |r| r.get(0),
            )
            .map_err(error)?;
        if !exists {
            return Err("运行不存在或不属于当前工作区。".into());
        }
        let revision: i64 = tx
            .query_row(
                "SELECT revision FROM execution_layouts WHERE run_id=?1",
                params![input.run_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(error)?
            .unwrap_or(0);
        if revision != input.expected_revision {
            return Err("视角已被另一处修改，本地视角已保留。请读取已保存视角后重试。".into());
        }
        tx.execute("INSERT INTO execution_layouts VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(run_id) DO UPDATE SET revision=excluded.revision,camera_json=excluded.camera_json,target_json=excluded.target_json,updated_at=excluded.updated_at",params![input.run_id,input.workspace_id,revision+1,serde_json::to_string(&input.camera).unwrap(),serde_json::to_string(&input.target).unwrap(),now()]).map_err(error)?;
        tx.commit().map_err(error)?;
        Ok(revision + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn views_are_scoped_revisioned_and_independent_of_execution_facts() {
        let directory = std::env::temp_dir().join(format!("cortexos-execution-{}", new_id()));
        let workspace;
        let run_id;
        let count;
        {
            let db = Database::open(&directory).unwrap();
            workspace = db.create_workspace("A").unwrap().id;
            let other = db.create_workspace("B").unwrap().id;
            let conversation = db.create_conversation(&workspace).unwrap().id;
            let run = db
                .prepare_run(
                    &new_id(),
                    &workspace,
                    &conversation,
                    "hello",
                    &ModelSettings::default(),
                )
                .unwrap();
            run_id = run.run.id.clone();
            db.finish(&run.run, "partial", "cancelled", None, json!({}))
                .unwrap();
            let graph = db.execution_graph(&workspace, &run_id).unwrap();
            count = graph.detail.events.len();
            assert_eq!(graph.output.content, "partial");
            assert_eq!(graph.detail.run.status, "cancelled");
            assert!(db.execution_graph(&other, &run_id).is_err());
            assert!(db.list_runs(&other, 0).unwrap().is_empty());
            assert_eq!(db.list_runs(&workspace, 0).unwrap().len(), 1);
            assert!(db.list_runs(&workspace, 100001).is_err());
            let view = SaveExecutionView {
                workspace_id: workspace.clone(),
                run_id: run_id.clone(),
                expected_revision: 0,
                camera: [1., 2., 30.],
                target: [1., 0., 0.],
            };
            let mut foreign = view.clone();
            foreign.workspace_id = other;
            assert!(db.save_execution_view(foreign).is_err());
            assert_eq!(db.save_execution_view(view.clone()).unwrap(), 1);
            assert!(db.save_execution_view(view.clone()).is_err());
            let mut invalid = view;
            invalid.expected_revision = 1;
            invalid.camera[0] = f64::NAN;
            assert!(db.save_execution_view(invalid).is_err());
            assert_eq!(
                db.run_detail(&workspace, &run_id).unwrap().events.len(),
                count
            );
            assert_eq!(db.knowledge_graph(&workspace).unwrap().view.revision, 0);
            for status in ["completed", "failed"] {
                let next = db
                    .prepare_run(
                        &new_id(),
                        &workspace,
                        &conversation,
                        "next",
                        &ModelSettings::default(),
                    )
                    .unwrap();
                db.finish(&next.run, "saved", status, None, json!({}))
                    .unwrap();
                assert_eq!(
                    db.execution_graph(&workspace, &next.run.id)
                        .unwrap()
                        .detail
                        .run
                        .status,
                    status
                );
            }
            db.prepare_run(
                &new_id(),
                &workspace,
                &conversation,
                "interrupted",
                &ModelSettings::default(),
            )
            .unwrap();
        }
        {
            let db = Database::open(&directory).unwrap();
            let graph = db.execution_graph(&workspace, &run_id).unwrap();
            assert_eq!(graph.view.revision, 1);
            assert_eq!(graph.view.camera, [1., 2., 30.]);
            assert_eq!(graph.detail.events.len(), count);
            assert!(
                db.list_runs(&workspace, 0)
                    .unwrap()
                    .iter()
                    .any(|run| run.status == "interrupted")
            );
            assert!(
                !db.connection()
                    .unwrap()
                    .prepare("PRAGMA foreign_key_check")
                    .unwrap()
                    .exists([])
                    .unwrap()
            );
        }
        std::fs::remove_dir_all(directory).unwrap();
    }
}
