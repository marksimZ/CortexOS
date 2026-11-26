use super::{
    Package, XML_ERROR, append, append_line, attr, nonempty, package_path, paragraphs, section,
    xml_events,
};
use crate::models::AppResult;
use quick_xml::events::Event;

pub(crate) fn extract_pptx(bytes: &[u8]) -> AppResult<String> {
    let mut package = Package::open(bytes, "pptx")?;
    let mut output = String::new();
    for (number, slide) in package.numbered("ppt/slides/slide") {
        let mut body = paragraphs(&package.read(&slide)?.ok_or(XML_ERROR)?, false, number)?;
        let filename = slide.rsplit('/').next().unwrap_or(&slide);
        let rels = format!("ppt/slides/_rels/{filename}.rels");
        let mut notes_path = None;
        if let Some(xml) = package.read(&rels)? {
            xml_events(&xml, |event| {
                if let Event::Start(tag) | Event::Empty(tag) = event {
                    if tag.local_name().as_ref() == "Relationship"
                        && attr(&tag, "TargetMode")?.as_deref() != Some("External")
                    {
                        if let Some(path) =
                            attr(&tag, "Target")?.and_then(|target| package_path(&slide, &target))
                        {
                            let filename = path.rsplit('/').next().unwrap_or(&path);
                            if (filename.starts_with("notesSlide") && filename.ends_with(".xml"))
                                || attr(&tag, "Type")?
                                    .is_some_and(|kind| kind.ends_with("/notesSlide"))
                            {
                                notes_path = Some(path);
                            }
                        }
                    }
                }
                Ok(())
            })?;
        }
        let notes = match notes_path {
            Some(path) if package.zip.index_for_name(&path).is_some() => package.read(&path)?,
            _ => package.read(&format!("ppt/notesSlides/notesSlide{number}.xml"))?,
        };
        if let Some(notes) = notes {
            let notes = paragraphs(&notes, true, number)?;
            for (index, line) in notes.lines().enumerate() {
                let prefix = if index == 0 {
                    "> 演讲者备注："
                } else {
                    "> "
                };
                append_line(&mut body, prefix)?;
                append(&mut body, line)?;
            }
        }
        if !body.is_empty() {
            section(&mut output, &format!("## 第 {number} 页\n"))?;
            append(&mut output, &body)?;
        }
    }
    nonempty(
        output,
        "该 .pptx 文件中没有可提取的文字内容（可能为纯图片课件）。",
    )
}
