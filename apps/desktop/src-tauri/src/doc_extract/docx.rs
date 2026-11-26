use super::{Package, nonempty, paragraphs};
use crate::models::AppResult;

pub(crate) fn extract_docx(bytes: &[u8]) -> AppResult<String> {
    let xml = Package::open(bytes, "docx")?
        .read("word/document.xml")?
        .ok_or(".docx 文件缺少正文内容。")?;
    nonempty(
        paragraphs(&xml, false, 0)?,
        ".docx 文档中没有可提取的文字内容。",
    )
}
