use crate::{
    benchmark::dataset::{self, DatasetSummary},
    db::{Database, error, json_error},
    models::AppResult,
};
use rusqlite::params;
use serde_json::{Value, json};
impl Database {
    pub fn personalized_datasets(&self, workspace: &str) -> AppResult<Vec<DatasetSummary>> {
        let c = self.connection()?;
        dataset::require_workspace(&c, workspace)?;
        let mut q=c.prepare("SELECT id,version,content_hash,manifest_json FROM benchmark_datasets WHERE json_extract(provenance_json,'$.workspaceId')=?1 ORDER BY created_at DESC LIMIT 100").map_err(error)?;
        q.query_map([workspace], |r| {
            let raw: String = r.get(3)?;
            Ok(DatasetSummary {
                id: r.get(0)?,
                version: r.get(1)?,
                content_hash: r.get(2)?,
                manifest: serde_json::from_str(&raw).map_err(|_| rusqlite::Error::InvalidQuery)?,
                runtime: Some(json!({"scoringVersion":2})),
            })
        })
        .map_err(error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(error)
    }
    pub fn personalized_dataset(
        &self,
        workspace: &str,
        id: &str,
        version: u32,
    ) -> AppResult<DatasetSummary> {
        let c = self.connection()?;
        let raw:(String,String)=c.query_row("SELECT content_hash,manifest_json FROM benchmark_datasets WHERE id=?1 AND version=?2 AND json_extract(provenance_json,'$.workspaceId')=?3",params![id,version,workspace],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|_|"测试集不存在或不属于此工作区。".to_string())?;
        let manifest: Value = serde_json::from_str(&raw.1).map_err(json_error)?;
        Ok(DatasetSummary {
            id: id.into(),
            version,
            content_hash: raw.0,
            manifest,
            runtime: Some(json!({"scoringVersion":2})),
        })
    }
    pub fn personalized_cases(
        &self,
        workspace: &str,
        id: &str,
        version: u32,
    ) -> AppResult<Vec<dataset::Case>> {
        let ds = self.personalized_dataset(workspace, id, version)?;
        let c = self.connection()?;
        let mut q=c.prepare("SELECT case_id FROM benchmark_cases WHERE dataset_id=?1 AND dataset_version=?2 ORDER BY rowid").map_err(error)?;
        let ids = q
            .query_map(params![id, version], |r| r.get::<_, String>(0))
            .map_err(error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(error)?;
        drop(q);
        drop(c);
        let cases = ids
            .iter()
            .map(|case| self.benchmark_case(workspace, id, version, case))
            .collect::<AppResult<Vec<_>>>()?;
        if dataset::hash(
            serde_json::to_string(&cases)
                .map_err(json_error)?
                .as_bytes(),
        ) != ds.content_hash
        {
            return Err("题库内容校验失败。".into());
        }
        Ok(cases)
    }
}
