use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const CHUNKING_VERSION: &str = "source-lines-v1";
const CHILD_BYTES: usize = 800;
const PARENT_BYTES: usize = 4800;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Location {
    line: usize,
    column: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Chunk {
    id: String,
    parent_chunk_id: Option<String>,
    content_hash: String,
    section_path: Vec<String>,
    symbol: Option<String>,
    start_location: Location,
    end_location: Location,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Structure {
    source_id: String,
    workspace_id: String,
    document_version_id: i64,
    source_path: String,
    chunking_version: &'static str,
    parents: Vec<Chunk>,
    children: Vec<Chunk>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StructureSummary {
    pub parent_count: usize,
    pub child_count: usize,
    pub sections: Vec<(String, usize)>,
}

impl Structure {
    pub(crate) fn summary(&self) -> StructureSummary {
        let mut sections = Vec::new();
        let mut previous = &[][..];
        for chunk in &self.parents {
            let path = chunk.section_path.as_slice();
            if !path.is_empty() && path != previous {
                sections.push((path.join(" / "), chunk.start_location.line));
            }
            previous = path;
        }
        StructureSummary {
            parent_count: self.parents.len(),
            child_count: self.children.len(),
            sections,
        }
    }
}

fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

type Section = (usize, usize, Vec<String>, Option<String>);

// Start-inclusive and end-exclusive byte offsets become one-based Unicode locations.
// Split long individual lines only at character boundaries and keep both sides addressable.
fn pieces(text: &str, bytes: usize) -> Vec<(usize, usize)> {
    let mut output = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let mut end = (start + bytes).min(text.len());
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        if end < text.len() {
            let window = &text[start..end];
            if let Some(newline) = window.rfind('\n').filter(|n| *n >= bytes / 2) {
                end = start + newline + 1;
            }
        }
        output.push((start, end));
        start = end;
    }
    output
}

pub(crate) struct Window {
    pub text: String,
    pub evidence: Value,
    pub matched: Value,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SegmentLocation {
    parent_chunk_id: String,
    parent_start: usize,
    parent_end: usize,
    start: usize,
    end: usize,
    start_location: Location,
    end_location: Location,
    section_path: Vec<String>,
    symbol: Option<String>,
    source_id: String,
    document_version_id: i64,
    content_hash: String,
}

pub(crate) struct Segment<'a> {
    pub id: String,
    pub body: &'a str,
    pub location: SegmentLocation,
}

fn segment_id(
    workspace: &str,
    source: &Value,
    kind: &str,
    start: usize,
    end: usize,
    hash: &str,
) -> String {
    digest(&format!(
        "{CHUNKING_VERSION}/{workspace}/{}/{}/{}/{kind}/{start}/{end}/{hash}",
        source["sourceId"], source["version"], source["startLine"]
    ))
}

pub(crate) fn segments<'a>(workspace: &str, content: &str, source: &'a Value) -> Vec<Segment<'a>> {
    let Some(raw) = source["excerpt"].as_str() else {
        return Vec::new();
    };
    // Only exact copies of approved ranges inherit the original file structure.
    if raw.trim() != content || raw.len() <= CHILD_BYTES {
        return Vec::new();
    }
    let Some(source_id) = source["sourceId"].as_str() else {
        return Vec::new();
    };
    let Some(version) = source["version"].as_i64() else {
        return Vec::new();
    };
    let Some(first_line) = source["startLine"].as_u64() else {
        return Vec::new();
    };
    let mut starts = vec![0];
    starts.extend(raw.match_indices('\n').map(|(offset, _)| offset + 1));
    let chars: Vec<_> = raw.char_indices().map(|(offset, _)| offset).collect();
    let original = |offset| {
        let mut point = location(&starts, &chars, offset);
        point.line += first_line as usize - 1;
        point
    };
    let mut output = Vec::new();
    for (a, b, section_path, symbol) in
        sections(raw, source["name"].as_str().is_some_and(is_markdown))
    {
        for (start, end) in pieces(&raw[a..b], PARENT_BYTES) {
            let (parent_start, parent_end) = (a + start, a + end);
            let parent_id = segment_id(
                workspace,
                source,
                "parent",
                parent_start,
                parent_end,
                &digest(&raw[parent_start..parent_end]),
            );
            for (start, end) in pieces(&raw[parent_start..parent_end], CHILD_BYTES) {
                let (start, end) = (parent_start + start, parent_start + end);
                let body = &raw[start..end];
                if body.trim().is_empty() {
                    continue;
                }
                let content_hash = digest(body);
                output.push(Segment {
                    id: segment_id(workspace, source, "child", start, end, &content_hash),
                    body,
                    location: SegmentLocation {
                        parent_chunk_id: parent_id.clone(),
                        parent_start,
                        parent_end,
                        start,
                        end,
                        start_location: original(start),
                        end_location: original(end),
                        content_hash,
                        section_path: section_path.clone(),
                        symbol: symbol.clone(),
                        source_id: source_id.into(),
                        document_version_id: version,
                    },
                });
            }
        }
    }
    output
}

pub(crate) fn windows(
    content: &str,
    source: &Value,
    chunk_id: &str,
    chunk: SegmentLocation,
) -> Option<[Window; 2]> {
    let raw = source["excerpt"].as_str()?;
    if raw.trim() != content
        || source["sourceId"] != chunk.source_id
        || source["version"] != chunk.document_version_id
        || !(chunk.parent_start <= chunk.start
            && chunk.start < chunk.end
            && chunk.end <= chunk.parent_end)
        || digest(raw.get(chunk.start..chunk.end)?) != chunk.content_hash
        || raw.get(chunk.parent_start..chunk.parent_end).is_none()
    {
        return None;
    }
    let first_line = source["startLine"].as_u64()?;
    let original_end = source["endLine"].as_u64()?;
    let line =
        |offset: usize| first_line + raw[..offset].bytes().filter(|c| *c == b'\n').count() as u64;
    let end_line = |start, end| {
        (line(start)
            + raw[start..end]
                .trim_end_matches('\n')
                .bytes()
                .filter(|c| *c == b'\n')
                .count() as u64)
            .min(original_end)
    };
    let fragment = |start, end, expanded| {
        let text = &raw[start..end];
        let mut evidence = source.clone();
        evidence["startLine"] = json!(line(start));
        evidence["endLine"] = json!(end_line(start, end));
        evidence["excerpt"] = json!(text);
        let matched = json!({
            "sourceId":chunk.source_id,"documentVersionId":chunk.document_version_id,"sourcePath":source["name"],
            "chunkId":chunk_id,"parentChunkId":chunk.parent_chunk_id,"chunkingVersion":CHUNKING_VERSION,
            "contentHash":chunk.content_hash,"sectionPath":chunk.section_path,"symbol":chunk.symbol,
            "startLocation":chunk.start_location,"endLocation":chunk.end_location,"expanded":expanded,
            "matchStartLine":line(chunk.start),"matchEndLine":end_line(chunk.start,chunk.end)
        });
        Window {
            text: text.into(),
            evidence,
            matched,
        }
    };
    Some([
        fragment(
            chunk.parent_start,
            chunk.parent_end,
            chunk.parent_start != chunk.start || chunk.parent_end != chunk.end,
        ),
        fragment(chunk.start, chunk.end, false),
    ])
}

pub(crate) fn stored_windows(
    connection: &rusqlite::Connection,
    workspace: &str,
    node: &str,
    version: i64,
    chunk_id: &str,
    content: &str,
    source: &Value,
) -> crate::models::AppResult<[Window; 2]> {
    let raw: String = connection.query_row(
        "SELECT location_json FROM rag_chunks WHERE workspace_id=?1 AND node_id=?2 AND version=?3 AND id=?4",
        rusqlite::params![workspace,node,version,chunk_id], |row| row.get(0)
    ).map_err(crate::db::error)?;
    serde_json::from_str(&raw)
        .ok()
        .and_then(|chunk| windows(content, source, chunk_id, chunk))
        .ok_or_else(|| "文件片段与原文版本不一致，请重新确认有效知识。".into())
}

fn is_markdown(name: &str) -> bool {
    name.rsplit_once('.')
        .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("md"))
}

fn location(starts: &[usize], chars: &[usize], offset: usize) -> Location {
    let line = starts.partition_point(|start| *start <= offset) - 1;
    let count_before = |byte| chars.partition_point(|character| *character < byte);
    Location {
        line: line + 1,
        column: count_before(offset) - count_before(starts[line]) + 1,
    }
}

fn heading(line: &str) -> Option<(usize, &str)> {
    let line = line
        .strip_prefix("   ")
        .or_else(|| line.strip_prefix("  "))
        .or_else(|| line.strip_prefix(' '))
        .unwrap_or(line);
    let level = line.bytes().take_while(|byte| *byte == b'#').count();
    let title = line.get(level..)?.trim().trim_end_matches('#').trim_end();
    ((1..=6).contains(&level) && line.as_bytes().get(level) == Some(&b' ') && !title.is_empty())
        .then_some((level, title))
}

fn code_symbol(line: &str) -> Option<&str> {
    let line = line.trim_start();
    let declaration = [
        "pub async fn ",
        "pub fn ",
        "async fn ",
        "fn ",
        "pub mod ",
        "mod ",
        "async def ",
        "def ",
        "class ",
        "export async function ",
        "export function ",
        "async function ",
        "function ",
        "export class ",
    ]
    .into_iter()
    .find_map(|prefix| line.strip_prefix(prefix))?;
    let length = declaration
        .char_indices()
        .find(|(_, c)| !(c.is_alphanumeric() || *c == '_'))
        .map_or(declaration.len(), |(index, _)| index);
    (length > 0).then_some(&declaration[..length])
}

fn marker(line: &str) -> Option<(u8, usize, &str)> {
    let first = *line.as_bytes().first()?;
    if !matches!(first, b'~' | b'`') {
        return None;
    }
    let length = line.bytes().take_while(|byte| *byte == first).count();
    (length >= 3).then(|| (first, length, line[length..].trim()))
}

fn sections(content: &str, markdown: bool) -> Vec<Section> {
    let mut offsets = 0;
    let mut start = 0;
    let mut paths: Vec<(usize, String)> = Vec::new();
    let mut symbol: Option<String> = None;
    let mut fence = None;
    let mut output = Vec::new();
    let mut flush = |start, end, next_path: &[(usize, String)], symbol: Option<String>| {
        if start < end {
            output.push((
                start,
                end,
                next_path.iter().map(|(_, title)| title.clone()).collect(),
                symbol,
            ));
        }
    };
    for line in content.split_inclusive('\n') {
        let text = line.trim_end();
        if markdown {
            if let Some((ch, size, info)) = marker(text.trim_start()) {
                match fence {
                    None => {
                        flush(start, offsets, &paths, symbol.take());
                        start = offsets;
                        fence = Some((ch, size));
                    }
                    Some((expected, opening))
                        if ch == expected && size >= opening && info.is_empty() =>
                    {
                        flush(start, offsets + line.len(), &paths, symbol.take());
                        start = offsets + line.len();
                        fence = None;
                    }
                    _ => {}
                }
            } else if fence.is_some() {
                if let Some(name) = code_symbol(text) {
                    if symbol.is_some() {
                        flush(start, offsets, &paths, symbol.take());
                        start = offsets;
                    }
                    symbol = Some(name.into());
                }
            } else if let Some((depth, title)) = heading(text) {
                flush(start, offsets, &paths, symbol.take());
                start = offsets;
                paths.retain(|(old, _)| *old < depth);
                paths.push((depth, title.chars().take(180).collect()));
            }
        } else if text.trim().is_empty() && offsets > start {
            flush(start, offsets + line.len(), &paths, None);
            start = offsets + line.len();
        }
        offsets += line.len();
    }
    flush(start, content.len(), &paths, symbol);
    output
}

pub(crate) fn build(
    workspace: &str,
    source: &str,
    version: i64,
    name: &str,
    content: &str,
) -> Structure {
    let markdown = is_markdown(name);
    let mut starts = vec![0];
    starts.extend(content.match_indices('\n').map(|(offset, _)| offset + 1));
    let chars: Vec<_> = content.char_indices().map(|(offset, _)| offset).collect();
    let mut parents = Vec::new();
    let mut children = Vec::new();
    for (start, end, path, symbol) in sections(content, markdown) {
        let item = |from, to, parent: Option<String>| {
            let content_hash = digest(&content[from..to]);
            let kind = if parent.is_some() { "child" } else { "parent" };
            let id = digest(&format!(
                "{CHUNKING_VERSION}/{workspace}/{source}/{version}/{kind}/{from}/{to}/{content_hash}"
            ));
            Chunk {
                id,
                parent_chunk_id: parent,
                content_hash,
                section_path: path.clone(),
                symbol: symbol.clone(),
                start_location: location(&starts, &chars, from),
                end_location: location(&starts, &chars, to),
            }
        };
        for (first, last) in pieces(&content[start..end], PARENT_BYTES) {
            let from = start + first;
            let to = start + last;
            if content[from..to].trim().is_empty() {
                continue;
            }
            let parent = item(from, to, None);
            for (a, b) in pieces(&content[from..to], CHILD_BYTES) {
                if !content[from + a..from + b].trim().is_empty() {
                    children.push(item(from + a, from + b, Some(parent.id.clone())));
                }
            }
            parents.push(parent);
        }
    }
    Structure {
        source_id: source.into(),
        workspace_id: workspace.into(),
        document_version_id: version,
        source_path: name.into(),
        chunking_version: CHUNKING_VERSION,
        parents,
        children,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_chunk_paths_symbols_and_version_identity() {
        let text = "# 发布\n说明\n## 操作\n~~~python\ndef first():\n    return 1\ndef second():\n    return 2\n~~~\n";
        let structure = build("w", "s", 1, "指南.md", text);
        assert_eq!(structure.parents.len(), 4);
        let function = &structure.children[2];
        assert_eq!(function.section_path, ["发布", "操作"]);
        assert_eq!(function.symbol.as_deref(), Some("first"));
        assert_eq!(function.start_location.line, 4);
        assert_eq!(function.end_location.line, 7);
        assert_eq!(
            function.parent_chunk_id.as_ref(),
            Some(&structure.parents[2].id)
        );
        assert_eq!(
            function.id,
            build("w", "s", 1, "指南.md", text).children[2].id
        );
        assert_ne!(
            function.id,
            build("w", "s", 2, "指南.md", text).children[2].id
        );
    }

    #[test]
    fn source_chunk_unicode_long_lines_and_paragraphs() {
        let text = format!("{}\n\n尾段", "🎛中文".repeat(600));
        let structure = build("w", "s", 3, "记录.txt", &text);
        assert!(structure.children.len() > 4);
        assert_eq!(structure.children[0].start_location.column, 1);
        assert!(structure.children[0].end_location.column > 1);
        assert_eq!(structure.children.last().unwrap().start_location.line, 3);
        for (start, end) in pieces(&text, CHILD_BYTES) {
            assert!(end - start <= CHILD_BYTES);
            assert!(text.is_char_boundary(start) && text.is_char_boundary(end));
        }
    }

    #[test]
    fn source_chunk_context_keeps_the_approved_range_and_edited_knowledge() {
        let raw = format!(
            "# 工程记录\n{}\n本机 SQLite 使用快照。\n相邻说明\n## 无关内容\n其它资料",
            "一般文字".repeat(240)
        );
        let source = json!({"sourceId":"fixture","version":2,"name":"项目.md","startLine":10,"endLine":15,"excerpt":raw});
        let segment = segments("w", &raw, &source)
            .into_iter()
            .find(|segment| segment.body.contains("SQLite"))
            .unwrap();
        let [parent, child] = windows(&raw, &source, &segment.id, segment.location).unwrap();
        assert_eq!(parent.evidence["startLine"], 10);
        assert_eq!(parent.evidence["endLine"], 13);
        assert_eq!(parent.matched["expanded"], true);
        assert_eq!(child.matched["matchEndLine"], 13);
        assert!(child.text.len() <= CHILD_BYTES);
        assert!(!parent.text.contains("无关内容"));
        assert!(segments("w", &format!("{raw}已编辑"), &source).is_empty());
    }
}
