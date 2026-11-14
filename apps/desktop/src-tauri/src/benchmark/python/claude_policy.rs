//! Separately approved Claude scope; shared file mechanics do not broaden Codex IDs.
use super::codex_policy;
use serde_json::{Value, json};
pub(crate) const COMPATIBLE: &str = "claude-bwrap-1";
const PROFILE: &str = include_str!("../../../catalogs/codex-seccomp/profile-v1.json");
const HASH: &str = "46a3ec985105a85a8d6bb5d21974a5dd5093f176411d766f8acf51bf5f2d9f13";
pub(crate) fn describe(id: &str) -> Result<Value, String> {
    match id {
        codex_policy::DEFAULT => Ok(json!({"id":"docker-default"})),
        COMPATIBLE if crate::benchmark::dataset::hash(PROFILE.as_bytes()) == HASH => Ok(
            json!({"id":COMPATIBLE,"hash":HASH,"sourceCommit":"4b8214fc4971e06172938a1f1554ccff6647ee80","scope":"claude_runtime_only","extraSyscalls":["unshare","clone","mount","umount2","pivot_root","chroot"]}),
        ),
        _ => Err("未知或指纹不匹配的Claude保护策略，已拒绝检查。".into()),
    }
}
pub(crate) fn create(id: &str, owner: &str) -> Result<Option<codex_policy::ProfileFile>, String> {
    describe(id)?;
    if id == codex_policy::DEFAULT {
        return Ok(None);
    }
    codex_policy::ProfileFile::create_fixed(owner, "claude", PROFILE, HASH).map(Some)
}
pub(crate) fn cleanup(owner: &str) -> Result<(), String> {
    codex_policy::cleanup_fixed_file(owner, "claude", HASH)
}
pub(crate) fn matches(config: &Value, id: &str) -> bool {
    if describe(id).is_err() {
        return false;
    }
    if id == codex_policy::DEFAULT {
        return codex_policy::matches(config, id);
    }
    let Some(options) = config["HostConfig"]["SecurityOpt"].as_array() else {
        return false;
    };
    let actual: Vec<_> = options
        .iter()
        .filter_map(Value::as_str)
        .filter_map(|v| v.strip_prefix("seccomp="))
        .collect();
    actual.len() == 1
        && serde_json::from_str::<Value>(actual[0]).ok()
            == serde_json::from_str::<Value>(PROFILE).ok()
}
