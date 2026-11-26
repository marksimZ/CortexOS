use super::{
    Package, XML_ERROR, append, attr, nonempty, package_path, section, xml_events, xml_text,
};
use crate::models::AppResult;
use quick_xml::{Reader, events::Event};
use std::collections::HashMap;

fn chapter_entries(package: &mut Package<'_>) -> AppResult<Vec<usize>> {
    let mut root = None;
    if let Some(container) = package.read("META-INF/container.xml")? {
        xml_events(&container, |event| {
            if let Event::Start(tag) | Event::Empty(tag) = event {
                if root.is_none() && tag.local_name().as_ref() == "rootfile" {
                    root = attr(&tag, "full-path")?.and_then(|path| package_path("", &path));
                }
            }
            Ok(())
        })?;
    }
    if let Some(path) = root {
        if let Some(opf) = package.read(&path)? {
            let mut manifest = HashMap::new();
            let mut spine = Vec::new();
            xml_events(&opf, |event| {
                if let Event::Start(tag) | Event::Empty(tag) = event {
                    match tag.local_name().as_ref() {
                        "item" => {
                            if let (Some(id), Some(href)) = (attr(&tag, "id")?, attr(&tag, "href")?)
                            {
                                if let Some(chapter) = package_path(&path, &href) {
                                    if (is_html(&chapter)
                                        || attr(&tag, "media-type")?.is_some_and(|kind| {
                                            matches!(
                                                kind.as_str(),
                                                "application/xhtml+xml" | "text/html"
                                            )
                                        }))
                                        && let Some(index) = package.zip.index_for_name(&chapter)
                                    {
                                        manifest.insert(id, index);
                                    }
                                }
                            }
                        }
                        "itemref" => spine.extend(attr(&tag, "idref")?),
                        _ => {}
                    }
                }
                Ok(())
            })?;
            let chapters: Vec<_> = spine
                .iter()
                .filter_map(|id| manifest.get(id))
                .copied()
                .collect();
            if !chapters.is_empty() {
                return Ok(chapters);
            }
        }
    }
    let mut chapters: Vec<_> = package
        .zip
        .file_names()
        .filter(|name| is_html(name))
        .collect();
    chapters.sort_unstable();
    Ok(chapters
        .into_iter()
        .filter_map(|name| package.zip.index_for_name(name))
        .collect())
}

fn is_html(path: &str) -> bool {
    path.rsplit_once('.').is_some_and(|(_, extension)| {
        ["xhtml", "html", "htm"]
            .iter()
            .any(|kind| extension.eq_ignore_ascii_case(kind))
    })
}

fn chapter_markup(xml: &str) -> Reader<&[u8]> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().check_end_names = false;
    reader.config_mut().allow_unmatched_ends = true;
    reader.config_mut().allow_dangling_amp = true;
    reader
}

fn chapter_text(mut xml: &str) -> AppResult<String> {
    let mut reader = chapter_markup(xml);
    let mut output = String::new();
    let mut line = String::new();
    let mut heading = false;
    let mut pending_space = false;
    loop {
        let event = reader.read_event().map_err(|_| XML_ERROR)?;
        let (name, start) = match &event {
            Event::Start(tag) | Event::Empty(tag) => {
                (tag.local_name().as_ref().to_ascii_lowercase(), true)
            }
            Event::End(tag) => (tag.local_name().as_ref().to_ascii_lowercase(), false),
            Event::Eof => break,
            _ => {
                if let Some(text) = xml_text(&event, true)? {
                    for ch in text.chars() {
                        if ch.is_whitespace() {
                            pending_space = !line.is_empty();
                        } else {
                            if pending_space {
                                append(&mut line, " ")?;
                                pending_space = false;
                            }
                            append(&mut line, ch.encode_utf8(&mut [0; 4]))?;
                        }
                    }
                }
                continue;
            }
        };
        if matches!(name.as_str(), "script" | "style" | "head") && matches!(event, Event::Start(_))
        {
            // Raw HTML script/style bodies can contain '<' outside XML markup.
            let remaining = &xml[reader.buffer_position() as usize..];
            let close = [b"</".as_slice(), name.as_bytes()].concat();
            let Some(end) = remaining
                .as_bytes()
                .windows(close.len())
                .enumerate()
                .find_map(|(index, part)| {
                    (part.eq_ignore_ascii_case(&close)
                        && remaining
                            .as_bytes()
                            .get(index + close.len())
                            .is_some_and(|b| *b == b'>' || b.is_ascii_whitespace()))
                    .then_some(index)
                })
            else {
                break;
            };
            let Some((_, tail)) = remaining[end..].split_once('>') else {
                break;
            };
            xml = tail;
            reader = chapter_markup(xml);
            continue;
        }
        let is_heading = matches!(name.as_str(), "h1" | "h2" | "h3" | "h4" | "h5" | "h6");
        if is_heading || matches!(name.as_str(), "p" | "li" | "div" | "td" | "th" | "br") {
            flush_chapter_line(&mut output, &mut line, heading)?;
            pending_space = false;
            if is_heading {
                heading = start && !matches!(event, Event::Empty(_));
            }
        }
    }
    flush_chapter_line(&mut output, &mut line, heading)?;
    Ok(output)
}

fn flush_chapter_line(output: &mut String, line: &mut String, heading: bool) -> AppResult<()> {
    if !line.is_empty() {
        if heading {
            section(output, "## ")?;
            append(output, line)?;
        } else {
            section(output, line)?;
        }
        line.clear();
    }
    Ok(())
}

pub(crate) fn extract_epub(bytes: &[u8]) -> AppResult<String> {
    let mut package = Package::open(bytes, "epub")?;
    let mut output = String::new();
    for index in chapter_entries(&mut package)? {
        let chapter = chapter_text(&package.read_index(index)?)?;
        if !chapter.is_empty() {
            section(&mut output, &chapter)?;
        }
    }
    nonempty(output, "该 .epub 电子书中没有可提取的正文内容。")
}
