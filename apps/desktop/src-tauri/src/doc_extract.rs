use crate::models::AppResult;
use quick_xml::{
    Reader, XmlVersion,
    events::{BytesStart, Event},
};
use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap, HashSet},
    io::{Cursor, Read},
};

const MAX_TEXT_BYTES: usize = 2 * 1024 * 1024;
const MAX_XML_BYTES: u64 = 4 * 1024 * 1024;
const MAX_INFLATED_BYTES: usize = 16 * 1024 * 1024;
const MAX_DOCUMENT_WORK: usize = 64 * 1024 * 1024;
const XML_ERROR: &str = "文档 XML 格式或编码异常。";
const TEXT_LIMIT: &str = "提取的文字超过 2 MiB，请拆分文档后重试。";

fn append(output: &mut String, text: &str) -> AppResult<()> {
    if text.len() > MAX_TEXT_BYTES - output.len() {
        return Err(TEXT_LIMIT.into());
    }
    output.push_str(text);
    Ok(())
}

fn section(output: &mut String, text: &str) -> AppResult<()> {
    if !output.is_empty() {
        append(output, "\n\n")?;
    }
    append(output, text)
}

fn nonempty(output: String, message: &str) -> AppResult<String> {
    if output.trim().is_empty() {
        Err(message.into())
    } else {
        Ok(output)
    }
}

struct Package<'a> {
    zip: zip::ZipArchive<Cursor<&'a [u8]>>,
    remaining: usize,
}

impl<'a> Package<'a> {
    fn open(bytes: &'a [u8], extension: &str) -> AppResult<Self> {
        let zip = zip::ZipArchive::new(Cursor::new(bytes))
            .map_err(|_| format!("无法解析 .{extension} 文档结构。"))?;
        Ok(Self {
            zip,
            remaining: MAX_DOCUMENT_WORK,
        })
    }

    fn read(&mut self, name: &str) -> AppResult<Option<String>> {
        self.zip
            .index_for_name(name)
            .map(|index| self.read_index(index))
            .transpose()
    }

    fn read_index(&mut self, index: usize) -> AppResult<String> {
        let file = self
            .zip
            .by_index(index)
            .map_err(|_| "无法打开文档中的 XML 文件。")?;
        let mut bytes = Vec::new();
        file.take((MAX_XML_BYTES + 1).min(self.remaining as u64 + 1))
            .read_to_end(&mut bytes)
            .map_err(|_| "读取文档 XML 失败。")?;
        if bytes.len() as u64 > MAX_XML_BYTES || bytes.len() > self.remaining {
            return Err("文档解压后的内容过大，请拆分后重试。".into());
        }
        self.remaining -= bytes.len();
        String::from_utf8(bytes).map_err(|_| XML_ERROR.into())
    }

    fn numbered(&self, prefix: &str) -> Vec<(u32, String)> {
        let mut names: Vec<_> = self
            .zip
            .file_names()
            .filter_map(|name| {
                let number = name.strip_prefix(prefix)?.strip_suffix(".xml")?;
                if !number.bytes().all(|b| b.is_ascii_digit()) {
                    return None;
                }
                Some((number.parse().ok()?, name.to_owned()))
            })
            .collect();
        names.sort_unstable();
        names
    }
}

fn xml_events(xml: &str, mut visit: impl FnMut(Event<'_>) -> AppResult<()>) -> AppResult<()> {
    let mut reader = Reader::from_str(xml);
    loop {
        match reader.read_event().map_err(|_| XML_ERROR)? {
            Event::Eof => return Ok(()),
            event => visit(event)?,
        }
    }
}

fn attr(tag: &BytesStart<'_>, key: &str) -> AppResult<Option<String>> {
    tag.try_get_attribute(key)
        .map_err(|_| XML_ERROR)?
        .map(|attr| {
            attr.normalized_value(XmlVersion::Implicit1_0)
                .map(Cow::into_owned)
                .map_err(|_| XML_ERROR.into())
        })
        .transpose()
}

fn xml_text<'a>(event: &Event<'a>, html: bool) -> AppResult<Option<Cow<'a, str>>> {
    let text = match event {
        Event::Text(text) => text.xml_content(XmlVersion::Implicit1_0),
        Event::CData(text) => text.xml_content(XmlVersion::Implicit1_0),
        Event::GeneralRef(reference) => {
            if let Some(ch) = reference.resolve_char_ref().map_err(|_| XML_ERROR)? {
                return Ok(Some(Cow::Owned(ch.to_string())));
            }
            let name = reference.xml_content(XmlVersion::Implicit1_0);
            if let Some(entity) = quick_xml::escape::resolve_xml_entity(&name) {
                Cow::Borrowed(entity)
            } else if html {
                match name.as_ref() {
                    "nbsp" => Cow::Borrowed(" "),
                    "ndash" => Cow::Borrowed("–"),
                    "mdash" => Cow::Borrowed("—"),
                    "hellip" => Cow::Borrowed("…"),
                    "lsquo" => Cow::Borrowed("‘"),
                    "rsquo" => Cow::Borrowed("’"),
                    "ldquo" => Cow::Borrowed("“"),
                    "rdquo" => Cow::Borrowed("”"),
                    _ => Cow::Owned(format!("&{name};")),
                }
            } else {
                return Err(XML_ERROR.into());
            }
        }
        _ => return Ok(None),
    };
    Ok(Some(text))
}

// Resolve relationship targets within the archive using ZIP's forward-slash paths.
fn package_path(base: &str, target: &str) -> Option<String> {
    let mut parts = Vec::new();
    if !target.starts_with('/') {
        parts.extend(
            base.rsplit_once('/')
                .into_iter()
                .flat_map(|(dir, _)| dir.split('/')),
        );
    }
    let target = target.split(['#', '?']).next()?;
    let mut decoded = Vec::new();
    let mut bytes = target.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            decoded.push(hex_digit(bytes.next()?)? * 16 + hex_digit(bytes.next()?)?);
        } else {
            decoded.push(byte);
        }
    }
    let decoded = std::str::from_utf8(&decoded).ok()?;
    if decoded.contains(['\\', ':', '\0']) {
        return None;
    }
    for part in decoded.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            part => parts.push(part),
        }
    }
    Some(parts.join("/"))
}

fn paragraphs(xml: &str, notes: bool, slide_number: u32) -> AppResult<String> {
    let mut result = String::new();
    let mut paragraph = String::new();
    let mut shape = String::new();
    let mut in_text = false;
    let mut slide_number_placeholder = false;
    let mut skip_field = false;
    xml_events(xml, |event| {
        match &event {
            Event::Start(tag) | Event::Empty(tag) => match tag.local_name().as_ref() {
                "t" => in_text = matches!(event, Event::Start(_)),
                "br" => append(&mut paragraph, "\n")?,
                "tab" => append(&mut paragraph, "\t")?,
                "ph" if notes => {
                    slide_number_placeholder |= attr(tag, "type")?.as_deref() == Some("sldNum")
                }
                "fld" if notes && matches!(event, Event::Start(_)) => {
                    skip_field = attr(tag, "type")?.as_deref() == Some("slidenum");
                }
                _ => {}
            },
            Event::End(tag) => match tag.local_name().as_ref() {
                "t" => in_text = false,
                "fld" => skip_field = false,
                "p" => {
                    let value = paragraph.trim_end();
                    if !value.trim().is_empty()
                        && !(notes && value.trim().parse::<u32>() == Ok(slide_number))
                    {
                        let target = if notes { &mut shape } else { &mut result };
                        append_line(target, value)?;
                    }
                    paragraph.clear();
                }
                "sp" if notes => {
                    if !slide_number_placeholder && !shape.is_empty() {
                        append_line(&mut result, &shape)?;
                    }
                    shape.clear();
                    slide_number_placeholder = false;
                }
                _ => {}
            },
            _ if in_text && !skip_field => {
                if let Some(text) = xml_text(&event, false)? {
                    append(&mut paragraph, &text)?;
                }
            }
            _ => {}
        }
        Ok(())
    })?;
    if !shape.is_empty() && !slide_number_placeholder {
        append_line(&mut result, &shape)?;
    }
    if !paragraph.trim().is_empty() && !slide_number_placeholder {
        append_line(&mut result, paragraph.trim_end())?;
    }
    Ok(result)
}

fn append_line(output: &mut String, line: &str) -> AppResult<()> {
    if !output.is_empty() {
        append(output, "\n")?;
    }
    append(output, line)
}

pub(crate) fn extract_docx(bytes: &[u8]) -> AppResult<String> {
    let xml = Package::open(bytes, "docx")?
        .read("word/document.xml")?
        .ok_or(".docx 文件缺少正文内容。")?;
    nonempty(
        paragraphs(&xml, false, 0)?,
        ".docx 文档中没有可提取的文字内容。",
    )
}

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

fn shared_strings(xml: &str) -> AppResult<Vec<String>> {
    let mut strings = Vec::new();
    let mut value = None;
    let mut in_text = false;
    xml_events(xml, |event| {
        match &event {
            Event::Start(tag) if tag.local_name().as_ref() == "si" => value = Some(String::new()),
            Event::Empty(tag) if tag.local_name().as_ref() == "si" => strings.push(String::new()),
            Event::Start(tag) if tag.local_name().as_ref() == "t" => in_text = true,
            Event::End(tag) if tag.local_name().as_ref() == "t" => in_text = false,
            Event::End(tag) if tag.local_name().as_ref() == "si" => {
                if let Some(text) = value.take() {
                    strings.push(text);
                }
            }
            _ if in_text => {
                if let (Some(value), Some(text)) = (&mut value, xml_text(&event, false)?) {
                    append(value, &text)?;
                }
            }
            _ => {}
        }
        Ok(())
    })?;
    Ok(strings)
}

fn sheet_paths(package: &mut Package<'_>) -> AppResult<Vec<(String, String)>> {
    let fallback = package.numbered("xl/worksheets/sheet");
    let mut targets = HashMap::new();
    if let Some(xml) = package.read("xl/_rels/workbook.xml.rels")? {
        xml_events(&xml, |event| {
            if let Event::Start(tag) | Event::Empty(tag) = event {
                if tag.local_name().as_ref() == "Relationship"
                    && attr(&tag, "TargetMode")?.as_deref() != Some("External")
                {
                    if let (Some(id), Some(path)) = (attr(&tag, "Id")?, attr(&tag, "Target")?) {
                        if let Some(path) = package_path("xl/workbook.xml", &path) {
                            targets.insert(id, path);
                        }
                    }
                }
            }
            Ok(())
        })?;
    }
    let mut sheets = Vec::new();
    let mut names = Vec::new();
    let mut seen = HashSet::new();
    if let Some(xml) = package.read("xl/workbook.xml")? {
        xml_events(&xml, |event| {
            if let Event::Start(tag) | Event::Empty(tag) = event {
                if tag.local_name().as_ref() == "sheet" {
                    let name = attr(&tag, "name")?;
                    let id = attr(&tag, "r:id")?;
                    let path = id.as_ref().and_then(|id| targets.get(id)).or_else(|| {
                        if targets.is_empty() {
                            fallback.get(names.len()).map(|(_, path)| path)
                        } else {
                            None
                        }
                    });
                    if let Some(path) =
                        path.filter(|path| package.zip.index_for_name(path).is_some())
                    {
                        if seen.insert(path.clone()) {
                            sheets.push((
                                name.clone()
                                    .unwrap_or_else(|| format!("Sheet {}", names.len() + 1)),
                                path.clone(),
                            ));
                        }
                    }
                    names.push(name);
                }
            }
            Ok(())
        })?;
    }
    let use_fallback_names = targets.is_empty() || sheets.is_empty();
    for (index, (number, path)) in fallback.into_iter().enumerate() {
        if seen.insert(path.clone()) {
            let name = if use_fallback_names {
                names.get(index).cloned().flatten()
            } else {
                None
            };
            sheets.push((name.unwrap_or_else(|| format!("Sheet {number}")), path));
        }
    }
    Ok(sheets)
}

const MAX_COLUMNS: usize = 50;
const MAX_ROWS: usize = 2000;

fn column(reference: &str) -> Option<usize> {
    let mut number = 0usize;
    for letter in reference.bytes().take_while(u8::is_ascii_uppercase) {
        number = number
            .checked_mul(26)?
            .checked_add((letter - b'A' + 1) as usize)?;
    }
    number.checked_sub(1)
}

fn worksheet<'a>(xml: &str, shared: &'a [String]) -> AppResult<Vec<Vec<Cow<'a, str>>>> {
    let mut rows = Vec::new();
    let mut row = None;
    let mut count = 0;
    let mut next_column = 0;
    let mut cell = None;
    let mut capture = false;
    xml_events(xml, |event| {
        match &event {
            Event::Start(tag) | Event::Empty(tag) => match tag.local_name().as_ref() {
                "row" => {
                    count += 1;
                    if count <= MAX_ROWS && matches!(event, Event::Start(_)) {
                        row = Some(Vec::new());
                        next_column = 0;
                    }
                }
                "c" if row.is_some() => {
                    let index = attr(tag, "r")?
                        .as_deref()
                        .and_then(column)
                        .unwrap_or(next_column);
                    next_column = index.saturating_add(1);
                    if index < MAX_COLUMNS && matches!(event, Event::Start(_)) {
                        cell = Some((index, attr(tag, "t")?, String::new()));
                    }
                }
                "v" | "t" if matches!(event, Event::Start(_)) => {
                    if let Some((_, kind, _)) = &cell {
                        capture = (tag.local_name().as_ref() == "t")
                            == (kind.as_deref() == Some("inlineStr"));
                    }
                }
                _ => {}
            },
            Event::End(tag) => match tag.local_name().as_ref() {
                "v" | "t" => capture = false,
                "c" => {
                    capture = false;
                    if let (Some(row), Some((index, kind, value))) = (&mut row, cell.take()) {
                        let value = if kind.as_deref() == Some("s") {
                            Cow::Borrowed(
                                value
                                    .trim()
                                    .parse::<usize>()
                                    .ok()
                                    .and_then(|index| shared.get(index))
                                    .map_or("", String::as_str),
                            )
                        } else {
                            Cow::Owned(value)
                        };
                        if !value.trim().is_empty() {
                            if row.len() <= index {
                                row.resize(index + 1, Cow::Borrowed(""));
                            }
                            row[index] = value;
                        }
                    }
                }
                "row" => {
                    if let Some(row) = row.take().filter(|row| !row.is_empty()) {
                        rows.push(row);
                    }
                }
                _ => {}
            },
            _ if capture => {
                if let (Some((_, _, value)), Some(text)) = (&mut cell, xml_text(&event, false)?) {
                    append(value, &text)?;
                }
            }
            _ => {}
        }
        Ok(())
    })?;
    Ok(rows)
}

fn markdown_row(output: &mut String, cells: &[Cow<'_, str>], columns: usize) -> AppResult<()> {
    append(output, "|")?;
    for index in 0..columns {
        append(output, " ")?;
        if let Some(cell) = cells.get(index) {
            let mut chars = cell.chars().peekable();
            while let Some(ch) = chars.next() {
                match ch {
                    '\\' => append(output, "\\\\")?,
                    '|' => append(output, "\\|")?,
                    '\r' | '\n' => {
                        if ch == '\r' && chars.peek() == Some(&'\n') {
                            chars.next();
                        }
                        append(output, "<br>")?;
                    }
                    _ => append(output, ch.encode_utf8(&mut [0; 4]))?,
                }
            }
        }
        append(output, " |")?;
    }
    append(output, "\n")
}

pub(crate) fn extract_xlsx(bytes: &[u8]) -> AppResult<String> {
    let mut package = Package::open(bytes, "xlsx")?;
    let shared = package
        .read("xl/sharedStrings.xml")?
        .map(|xml| shared_strings(&xml))
        .transpose()?
        .unwrap_or_default();
    let mut output = String::new();
    for (name, path) in sheet_paths(&mut package)? {
        let Some(xml) = package.read(&path)? else {
            continue;
        };
        let rows = worksheet(&xml, &shared)?;
        let Some(columns) = rows.iter().map(Vec::len).max() else {
            continue;
        };
        section(&mut output, &format!("## {name}\n"))?;
        markdown_row(&mut output, &rows[0], columns)?;
        markdown_row(&mut output, &vec![Cow::Borrowed("---"); columns], columns)?;
        for row in &rows[1..] {
            markdown_row(&mut output, row, columns)?;
        }
        output.pop();
    }
    nonempty(output, "该 .xlsx 表格中没有可提取的文字内容。")
}

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

// RFC 1950 framing and RFC 1951 blocks. Numeric fields are packed low bit first.
struct Bits<'a> {
    data: &'a [u8],
    position: usize,
}

impl Bits<'_> {
    fn read(&mut self, count: usize) -> Option<usize> {
        let mut value = 0;
        for bit in 0..count {
            let byte = self.data.get(self.position / 8)?;
            value |= (((byte >> (self.position % 8)) & 1) as usize) << bit;
            self.position += 1;
        }
        Some(value)
    }

    fn bytes(&mut self, count: usize) -> Option<&[u8]> {
        let start = self.position.div_ceil(8);
        let end = start.checked_add(count)?;
        let bytes = self.data.get(start..end)?;
        self.position = end * 8;
        Some(bytes)
    }
}

struct Huffman {
    counts: [usize; 16],
    symbols: Vec<usize>,
    max_bits: usize,
}

impl Huffman {
    fn new(lengths: &[usize]) -> Option<Self> {
        let mut counts = [0; 16];
        for &length in lengths {
            if length != 0 {
                *counts.get_mut(length)? += 1;
            }
        }
        let mut remaining = 1usize;
        for count in &counts[1..] {
            remaining = (remaining * 2).checked_sub(*count)?;
        }
        let max_bits = counts.iter().rposition(|&count| count > 0).unwrap_or(0);
        let mut offsets = [0; 16];
        for length in 1..16 {
            offsets[length] = offsets[length - 1] + counts[length - 1];
        }
        let mut symbols = vec![0; counts.iter().sum()];
        for (symbol, &length) in lengths.iter().enumerate() {
            if length != 0 {
                symbols[offsets[length]] = symbol;
                offsets[length] += 1;
            }
        }
        Some(Self {
            counts,
            symbols,
            max_bits,
        })
    }

    fn read(&self, bits: &mut Bits<'_>) -> Option<usize> {
        // Canonical Huffman codes are read most-significant bit first.
        let (mut code, mut first, mut offset) = (0, 0, 0);
        for length in 1..=self.max_bits {
            code = (code << 1) | bits.read(1)?;
            let count = self.counts[length];
            if (first..first + count).contains(&code) {
                return self.symbols.get(offset + code - first).copied();
            }
            offset += count;
            first = (first + count) << 1;
        }
        None
    }
}

fn dynamic_codes(bits: &mut Bits<'_>) -> Option<(Huffman, Huffman)> {
    const ORDER: [usize; 19] = [
        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
    ];
    let literal_count = bits.read(5)? + 257;
    let distance_count = bits.read(5)? + 1;
    let length_count = bits.read(4)? + 4;
    if literal_count > 286 {
        return None;
    }
    let mut alphabet = [0; 19];
    for &index in &ORDER[..length_count] {
        alphabet[index] = bits.read(3)?;
    }
    let codes = Huffman::new(&alphabet)?;
    let total = literal_count + distance_count;
    let mut lengths = Vec::with_capacity(total);
    while lengths.len() < total {
        let (count, length) = match codes.read(bits)? {
            code @ 0..=15 => (1, code),
            16 => (bits.read(2)? + 3, *lengths.last()?),
            17 => (bits.read(3)? + 3, 0),
            18 => (bits.read(7)? + 11, 0),
            _ => return None,
        };
        if count > total - lengths.len() {
            return None;
        }
        lengths.resize(lengths.len() + count, length);
    }
    if lengths[256] == 0 {
        return None;
    }
    Some((
        Huffman::new(&lengths[..literal_count])?,
        Huffman::new(&lengths[literal_count..])?,
    ))
}

fn inflate_codes(
    bits: &mut Bits<'_>,
    output: &mut Vec<u8>,
    literals: &Huffman,
    distances: &Huffman,
    limit: usize,
    window: usize,
) -> Option<()> {
    const LENGTH_BASE: [usize; 29] = [
        3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115,
        131, 163, 195, 227, 258,
    ];
    const DISTANCE_BASE: [usize; 30] = [
        1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
        2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
    ];
    loop {
        match literals.read(bits)? {
            code @ 0..=255 if output.len() < limit => output.push(code as u8),
            256 => return Some(()),
            code @ 257..=285 => {
                let index = code - 257;
                let extra = if index == 28 {
                    0
                } else {
                    index.saturating_sub(4) / 4
                };
                let length = LENGTH_BASE[index] + bits.read(extra)?;
                let symbol = distances.read(bits)?;
                let distance =
                    DISTANCE_BASE.get(symbol)? + bits.read(symbol.saturating_sub(2) / 2)?;
                if length > limit - output.len() || distance > window || distance > output.len() {
                    return None;
                }
                for _ in 0..length {
                    output.push(output[output.len() - distance]);
                }
            }
            _ => return None,
        }
    }
}

fn adler32(bytes: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in bytes.chunks(5552) {
        for &byte in chunk {
            a += byte as u32;
            b += a;
        }
        a %= 65521;
        b %= 65521;
    }
    (b << 16) | a
}

fn inflate_zlib(data: &[u8]) -> Option<Vec<u8>> {
    let &[cmf, flags] = data.get(..2)? else {
        return None;
    };
    if cmf & 0x0f != 8 || cmf >> 4 > 7 || u16::from_be_bytes([cmf, flags]) % 31 != 0 {
        return None;
    }
    let window = 1 << ((cmf >> 4) + 8);
    let start = if flags & 0x20 == 0 { 2 } else { 6 };
    let end = data.len().checked_sub(4)?;
    let mut bits = Bits {
        data: data.get(start..end)?,
        position: 0,
    };
    let mut output = Vec::new();
    let fixed = std::cell::OnceCell::new();
    loop {
        let last = bits.read(1)? != 0;
        match bits.read(2)? {
            0 => {
                let header: [u8; 4] = bits.bytes(4)?.try_into().ok()?;
                let length = u16::from_le_bytes([header[0], header[1]]);
                if length != !u16::from_le_bytes([header[2], header[3]]) {
                    return None;
                }
                let length = length as usize;
                if length > MAX_INFLATED_BYTES - output.len() {
                    return None;
                }
                output.extend_from_slice(bits.bytes(length)?);
            }
            1 => {
                let (literals, distances) = fixed.get_or_init(|| {
                    let mut lengths = [8; 288];
                    lengths[144..256].fill(9);
                    lengths[256..280].fill(7);
                    (
                        Huffman::new(&lengths).unwrap(),
                        Huffman::new(&[5; 32]).unwrap(),
                    )
                });
                inflate_codes(
                    &mut bits,
                    &mut output,
                    literals,
                    distances,
                    MAX_INFLATED_BYTES,
                    window,
                )?;
            }
            2 => {
                let (literals, distances) = dynamic_codes(&mut bits)?;
                inflate_codes(
                    &mut bits,
                    &mut output,
                    &literals,
                    &distances,
                    MAX_INFLATED_BYTES,
                    window,
                )?;
            }
            _ => return None,
        }
        if last {
            let checksum = u32::from_be_bytes(data[end..].try_into().ok()?);
            return (bits.position.div_ceil(8) == bits.data.len() && adler32(&output) == checksum)
                .then_some(output);
        }
    }
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn pdf_space(byte: u8) -> bool {
    matches!(byte, 0 | b'\t' | b'\n' | 12 | b'\r' | b' ')
}

fn pdf_delimiter(byte: u8) -> bool {
    pdf_space(byte) || b"()<>[]{}/%".contains(&byte)
}

#[derive(Clone, Copy)]
struct PdfLexer<'a> {
    data: &'a [u8],
    pos: usize,
}

#[derive(Clone, Copy, PartialEq)]
enum PdfToken<'a> {
    Word(&'a [u8]),
    Name(&'a [u8]),
    Literal(&'a [u8]),
    Hex(&'a [u8]),
    DictStart,
    DictEnd,
    ArrayStart,
    ArrayEnd,
    Other,
}

impl<'a> PdfLexer<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn next(&mut self) -> Option<PdfToken<'a>> {
        let first = loop {
            let byte = *self.data.get(self.pos)?;
            self.pos += 1;
            if byte == b'%' {
                while self
                    .data
                    .get(self.pos)
                    .is_some_and(|b| !matches!(b, b'\r' | b'\n'))
                {
                    self.pos += 1;
                }
            } else if !pdf_space(byte) {
                break byte;
            }
        };
        let start = self.pos;
        let token = match first {
            b'[' => PdfToken::ArrayStart,
            b']' => PdfToken::ArrayEnd,
            b'<' if self.data.get(self.pos) == Some(&b'<') => {
                self.pos += 1;
                PdfToken::DictStart
            }
            b'>' if self.data.get(self.pos) == Some(&b'>') => {
                self.pos += 1;
                PdfToken::DictEnd
            }
            b'<' => {
                let end = self.data.get(self.pos..)?.iter().position(|&b| b == b'>');
                let Some(end) = end else {
                    self.pos = self.data.len();
                    return Some(PdfToken::Other);
                };
                self.pos += end + 1;
                PdfToken::Hex(&self.data[start..start + end])
            }
            b'(' => {
                let mut depth = 1;
                while let Some(&byte) = self.data.get(self.pos) {
                    self.pos += 1;
                    match byte {
                        b'\\' => self.pos = (self.pos + 1).min(self.data.len()),
                        b'(' => depth += 1,
                        b')' => {
                            depth -= 1;
                            if depth == 0 {
                                return Some(PdfToken::Literal(&self.data[start..self.pos - 1]));
                            }
                        }
                        _ => {}
                    }
                }
                PdfToken::Other
            }
            b'/' => {
                self.word_end();
                PdfToken::Name(&self.data[start..self.pos])
            }
            byte if !pdf_delimiter(byte) => {
                self.word_end();
                PdfToken::Word(&self.data[start - 1..self.pos])
            }
            _ => PdfToken::Other,
        };
        Some(token)
    }

    fn word_end(&mut self) {
        while self.data.get(self.pos).is_some_and(|&b| !pdf_delimiter(b)) {
            self.pos += 1;
        }
    }

    fn value(&mut self) -> Option<&'a [u8]> {
        let start = self.pos;
        match self.next()? {
            PdfToken::ArrayStart | PdfToken::DictStart => {
                let mut depth = 1;
                while depth > 0 {
                    match self.next()? {
                        PdfToken::ArrayStart | PdfToken::DictStart => depth += 1,
                        PdfToken::ArrayEnd | PdfToken::DictEnd => depth -= 1,
                        _ => {}
                    }
                }
            }
            token if uint(token).is_some() => {
                let mut lookahead = *self;
                if lookahead.next().and_then(uint).is_some()
                    && lookahead.next() == Some(PdfToken::Word(b"R"))
                {
                    self.pos = lookahead.pos;
                }
            }
            _ => {}
        }
        Some(&self.data[start..self.pos])
    }
}

fn uint(token: PdfToken<'_>) -> Option<u32> {
    let PdfToken::Word(word) = token else {
        return None;
    };
    std::str::from_utf8(word).ok()?.parse().ok()
}

fn number(token: PdfToken<'_>) -> Option<f64> {
    let PdfToken::Word(word) = token else {
        return None;
    };
    std::str::from_utf8(word)
        .ok()?
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite())
}

type ObjectId = (u32, u32);

fn pdf_ref(value: &[u8]) -> Option<ObjectId> {
    let mut lexer = PdfLexer::new(value);
    let id = (uint(lexer.next()?)?, uint(lexer.next()?)?);
    (lexer.next()? == PdfToken::Word(b"R")).then_some(id)
}

fn pdf_refs(value: &[u8]) -> Vec<ObjectId> {
    let mut lexer = PdfLexer::new(value);
    if lexer.next() != Some(PdfToken::ArrayStart) {
        return pdf_ref(value).into_iter().collect();
    }
    let mut refs = Vec::new();
    while let Some(value) = lexer.value() {
        if let Some(id) = pdf_ref(value) {
            refs.push(id);
        }
    }
    refs
}

fn pdf_dict(data: &[u8]) -> impl Iterator<Item = (&[u8], &[u8])> {
    let mut lexer = PdfLexer::new(data);
    let dict = lexer.next() == Some(PdfToken::DictStart);
    std::iter::from_fn(move || {
        if dict {
            if let Some(PdfToken::Name(key)) = lexer.next() {
                return Some((key, lexer.value()?));
            }
        }
        None
    })
    .fuse()
}

fn pdf_key<'a>(dictionary: &'a [u8], key: &[u8]) -> Option<&'a [u8]> {
    pdf_dict(dictionary).find_map(|(name, value)| (name == key).then_some(value))
}

fn pdf_type(data: &[u8], kind: &[u8]) -> bool {
    pdf_key(data, b"Type").and_then(|value| PdfLexer::new(value).next())
        == Some(PdfToken::Name(kind))
}

struct PdfObject<'a> {
    id: ObjectId,
    value: &'a [u8],
    stream: Option<&'a [u8]>,
}

struct Pdf<'a> {
    objects: Vec<PdfObject<'a>>,
    index: HashMap<ObjectId, usize>,
    encrypted: bool,
}

impl<'a> Pdf<'a> {
    fn parse(bytes: &'a [u8]) -> AppResult<Self> {
        let mut pdf = Self {
            objects: Vec::new(),
            index: HashMap::new(),
            encrypted: false,
        };
        let mut lexer = PdfLexer::new(bytes);
        while let Some(token) = lexer.next() {
            if token == PdfToken::Word(b"trailer") {
                if let Some(trailer) = lexer.value() {
                    pdf.encrypted |= pdf_key(trailer, b"Encrypt").is_some();
                }
                continue;
            }
            let Some(object) = uint(token) else { continue };
            let mut next = lexer;
            let Some(generation) = next.next().and_then(uint) else {
                continue;
            };
            if next.next() != Some(PdfToken::Word(b"obj")) {
                continue;
            }
            let Some(value) = next.value() else { break };
            let id = (object, generation);
            let value_end = next.pos;
            let stream = if next.next() == Some(PdfToken::Word(b"stream")) {
                let length = pdf_key(value, b"Length")
                    .and_then(|value| pdf.resolve(value))
                    .and_then(|value| PdfLexer::new(value).next())
                    .and_then(uint);
                pdf_stream(&mut next, length.map(|n| n as usize))
            } else {
                // Keep a following object header available after a malformed end marker.
                next.pos = value_end;
                None
            };
            lexer = next;
            pdf.encrypted |= pdf_key(value, b"Encrypt").is_some();
            if pdf.objects.len() >= 100_000 {
                return Err("PDF 对象过多，请拆分文档后重试。".into());
            }
            pdf.index.insert(id, pdf.objects.len());
            pdf.objects.push(PdfObject { id, value, stream });
        }
        Ok(pdf)
    }

    fn object(&self, id: ObjectId) -> Option<&PdfObject<'a>> {
        self.index
            .get(&id)
            .and_then(|&index| self.objects.get(index))
    }

    fn current_objects(&self) -> impl Iterator<Item = &PdfObject<'a>> {
        self.objects
            .iter()
            .enumerate()
            .filter(|(index, object)| self.index.get(&object.id) == Some(index))
            .map(|(_, object)| object)
    }

    fn resolve(&self, value: &'a [u8]) -> Option<&'a [u8]> {
        match pdf_ref(value) {
            Some(id) => Some(self.object(id)?.value),
            None => Some(value),
        }
    }

    fn pages(&self) -> Vec<&PdfObject<'a>> {
        let mut pages = Vec::new();
        let mut seen = HashSet::new();
        if let Some(root) = self
            .current_objects()
            .filter(|object| pdf_type(object.value, b"Catalog"))
            .last()
            .and_then(|catalog| pdf_key(catalog.value, b"Pages"))
            .and_then(pdf_ref)
        {
            let mut stack = vec![root];
            let mut expanded = HashSet::new();
            while let Some(id) = stack.pop() {
                if !expanded.insert(id) {
                    continue;
                }
                let Some(object) = self.object(id) else {
                    continue;
                };
                if pdf_type(object.value, b"Page") {
                    pages.push(object);
                    seen.insert(id);
                } else if let Some(kids) =
                    pdf_key(object.value, b"Kids").and_then(|value| self.resolve(value))
                {
                    stack.extend(pdf_refs(kids).into_iter().rev());
                }
            }
        }
        pages.extend(
            self.current_objects()
                .filter(|object| !seen.contains(&object.id) && pdf_type(object.value, b"Page")),
        );
        pages
    }

    fn fonts<'b>(
        &'b self,
        mut page: &'b PdfObject<'a>,
    ) -> HashMap<Cow<'a, [u8]>, Option<ObjectId>> {
        for _ in 0..64 {
            if let Some(resources) = pdf_key(page.value, b"Resources") {
                return self
                    .resolve(resources)
                    .and_then(|resources| pdf_key(resources, b"Font"))
                    .and_then(|fonts| self.resolve(fonts))
                    .map(|fonts| {
                        pdf_dict(fonts)
                            .map(|(name, value)| {
                                let cmap = self
                                    .resolve(value)
                                    .and_then(|font| pdf_key(font, b"ToUnicode"))
                                    .and_then(pdf_ref);
                                (pdf_name(name), cmap)
                            })
                            .collect()
                    })
                    .unwrap_or_default();
            }
            let Some(parent) = pdf_key(page.value, b"Parent")
                .and_then(pdf_ref)
                .and_then(|id| self.object(id))
            else {
                break;
            };
            page = parent;
        }
        HashMap::new()
    }
}

fn pdf_stream<'a>(lexer: &mut PdfLexer<'a>, length: Option<usize>) -> Option<&'a [u8]> {
    let tail = lexer.data.get(lexer.pos..)?;
    let tail = &tail[tail
        .iter()
        .take_while(|&&byte| matches!(byte, b' ' | b'\t'))
        .count()..];
    let payload = tail
        .strip_prefix(b"\r\n")
        .or_else(|| tail.strip_prefix(b"\n"))
        .or_else(|| tail.strip_prefix(b"\r"))?;
    let start = lexer.data.len() - payload.len();
    if let Some(end) = length.filter(|&end| end <= payload.len()) {
        let mut check = PdfLexer {
            data: lexer.data,
            pos: start + end,
        };
        if check.next() == Some(PdfToken::Word(b"endstream")) {
            lexer.pos = check.pos;
            return Some(&payload[..end]);
        }
    }
    let marker = b"endstream";
    let Some(end) = payload
        .windows(marker.len())
        .enumerate()
        .find_map(|(index, bytes)| {
            if bytes != marker || (index != 0 && !matches!(payload[index - 1], b'\r' | b'\n')) {
                return None;
            }
            let mut check = PdfLexer::new(&payload[index + marker.len()..]);
            (check.next() == Some(PdfToken::Word(b"endobj"))).then_some(index)
        })
    else {
        lexer.pos = lexer.data.len();
        return None;
    };
    lexer.pos = start + end + marker.len();
    let data = &payload[..end];
    Some(
        data.strip_suffix(b"\r\n")
            .or_else(|| data.strip_suffix(b"\n"))
            .or_else(|| data.strip_suffix(b"\r"))
            .unwrap_or(data),
    )
}

fn decoded_stream<'a>(
    object: &PdfObject<'a>,
    budget: &mut usize,
) -> AppResult<Option<Cow<'a, [u8]>>> {
    let Some(data) = object.stream else {
        return Ok(None);
    };
    let result = match pdf_key(object.value, b"Filter") {
        None => Cow::Borrowed(data),
        Some(filter) => {
            let mut lexer = PdfLexer::new(filter);
            let flate = |token| matches!(token, Some(PdfToken::Name(b"FlateDecode" | b"Fl")));
            let first = lexer.next();
            if !flate(first)
                && !(first == Some(PdfToken::ArrayStart)
                    && flate(lexer.next())
                    && lexer.next() == Some(PdfToken::ArrayEnd))
            {
                return Ok(None);
            }
            let Some(decoded) = inflate_zlib(data) else {
                *budget = budget
                    .checked_sub(MAX_INFLATED_BYTES)
                    .ok_or("PDF 解压后的数据过大。")?;
                return Ok(None);
            };
            Cow::Owned(decoded)
        }
    };
    *budget = budget
        .checked_sub(result.len().max(1))
        .ok_or("PDF 解压后的数据过大，请拆分后重试。")?;
    Ok(Some(result))
}

fn hex_bytes(input: &[u8]) -> Option<Vec<u8>> {
    let mut output = Vec::new();
    let mut high = None;
    for &byte in input.iter().filter(|&&byte| !pdf_space(byte)) {
        let nibble = hex_digit(byte)?;
        if let Some(high) = high.take() {
            output.push(high * 16 + nibble);
        } else {
            high = Some(nibble);
        }
    }
    if let Some(high) = high {
        output.push(high * 16);
    }
    Some(output)
}

fn literal_bytes(input: &[u8]) -> Vec<u8> {
    let mut output = Vec::new();
    let mut bytes = input.iter().copied().peekable();
    while let Some(byte) = bytes.next() {
        let byte = match byte {
            b'\\' => match bytes.next() {
                Some(b'n') => b'\n',
                Some(b'r') => b'\r',
                Some(b't') => b'\t',
                Some(b'b') => 8,
                Some(b'f') => 12,
                Some(b'\r') => {
                    bytes.next_if_eq(&b'\n');
                    continue;
                }
                Some(b'\n') => continue,
                Some(first @ b'0'..=b'7') => {
                    let mut value = first - b'0';
                    for _ in 0..2 {
                        let Some(digit) = bytes.next_if(|b| (b'0'..=b'7').contains(b)) else {
                            break;
                        };
                        value = value.wrapping_mul(8).wrapping_add(digit - b'0');
                    }
                    value
                }
                Some(other) => other,
                None => break,
            },
            b'\r' => {
                bytes.next_if_eq(&b'\n');
                b'\n'
            }
            other => other,
        };
        output.push(byte);
    }
    output
}

fn utf16_be(bytes: &[u8]) -> impl Iterator<Item = char> + '_ {
    char::decode_utf16(
        bytes
            .chunks_exact(2)
            .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]])),
    )
    .map(|ch| ch.unwrap_or(char::REPLACEMENT_CHARACTER))
}

struct CodeRange {
    end: u32,
    target: Vec<u8>,
}

#[derive(Default)]
struct CMap {
    ranges: BTreeMap<(usize, u32), CodeRange>,
    widths: [bool; 4],
}

fn cmap_source(token: PdfToken<'_>) -> Option<(usize, u32)> {
    let PdfToken::Hex(hex) = token else {
        return None;
    };
    if hex.len() > 32 {
        return None;
    }
    let bytes = hex_bytes(hex)?;
    if !(1..=4).contains(&bytes.len()) {
        return None;
    }
    let code = bytes
        .iter()
        .fold(0, |code, &byte| (code << 8) | u32::from(byte));
    Some((bytes.len(), code))
}

impl CMap {
    fn insert(&mut self, width: usize, start: u32, end: u32, hex: &[u8]) {
        if self.ranges.len() >= 65_536 || hex.len() > 512 {
            return;
        }
        if let Some(target) =
            hex_bytes(hex).filter(|bytes| !bytes.is_empty() && bytes.len() % 2 == 0)
        {
            self.ranges
                .insert((width, start), CodeRange { end, target });
            self.widths[width - 1] = true;
        }
    }

    fn parse(bytes: &[u8]) -> Self {
        let mut cmap = Self::default();
        let mut lexer = PdfLexer::new(bytes);
        while let Some(token) = lexer.next() {
            match token {
                PdfToken::Word(b"beginbfchar") => {
                    while let Some((width, code)) = lexer.next().and_then(cmap_source) {
                        if let Some(PdfToken::Hex(target)) = lexer.next() {
                            cmap.insert(width, code, code, target);
                        } else {
                            break;
                        }
                    }
                }
                PdfToken::Word(b"beginbfrange") => {
                    while let Some((width, start)) = lexer.next().and_then(cmap_source) {
                        let Some((end_width, end)) = lexer.next().and_then(cmap_source) else {
                            break;
                        };
                        let valid = width == end_width && start <= end;
                        match lexer.next() {
                            Some(PdfToken::Hex(target)) if valid => {
                                cmap.insert(width, start, end, target)
                            }
                            Some(PdfToken::Hex(_)) => {}
                            Some(PdfToken::ArrayStart) => {
                                let mut code = Some(start);
                                while let Some(PdfToken::Hex(target)) = lexer.next() {
                                    if let Some(index) = code.filter(|&index| valid && index <= end)
                                    {
                                        cmap.insert(width, index, index, target);
                                        code = index.checked_add(1);
                                    }
                                }
                            }
                            _ => break,
                        }
                    }
                }
                _ => {}
            }
        }
        cmap
    }

    fn lookup(&self, bytes: &[u8]) -> Option<(usize, &CodeRange, u32)> {
        for (index, &enabled) in self.widths.iter().enumerate().rev() {
            let width = index + 1;
            if !enabled || width > bytes.len() {
                continue;
            }
            let code = bytes[..width]
                .iter()
                .fold(0, |code, &byte| (code << 8) | u32::from(byte));
            if let Some((&(found_width, start), range)) =
                self.ranges.range(..=(width, code)).next_back()
            {
                if width == found_width && code <= range.end {
                    return Some((width, range, code - start));
                }
            }
        }
        None
    }
}

fn append_pdf_char(output: &mut String, ch: char) -> AppResult<()> {
    match ch {
        '\r' => append(output, "\n"),
        '\t' | '\n' => append(output, ch.encode_utf8(&mut [0; 4])),
        _ if ch.is_control() || ch == '\u{feff}' => Ok(()),
        _ => append(output, ch.encode_utf8(&mut [0; 4])),
    }
}

fn pdf_doc_char(byte: u8) -> char {
    const ACCENTS: [char; 8] = ['˘', 'ˇ', 'ˆ', '˙', '˝', '˛', '˚', '˜'];
    const TYPOGRAPHY: [char; 31] = [
        '•', '†', '‡', '…', '—', '–', 'ƒ', '⁄', '‹', '›', '−', '‰', '„', '“', '”', '‘', '’', '‚',
        '™', 'ﬁ', 'ﬂ', 'Ł', 'Œ', 'Š', 'Ÿ', 'Ž', 'ı', 'ł', 'œ', 'š', 'ž',
    ];
    match byte {
        0x18..=0x1f => ACCENTS[(byte - 0x18) as usize],
        0x80..=0x9e => TYPOGRAPHY[(byte - 0x80) as usize],
        0xa0 => '€',
        _ => char::from(byte),
    }
}

fn looks_utf16_be(bytes: &[u8]) -> bool {
    if bytes.is_empty() || bytes.len() % 2 != 0 {
        return false;
    }
    let mut cjk = false;
    let mut ascii = false;
    for pair in bytes.chunks_exact(2) {
        let value = u16::from_be_bytes([pair[0], pair[1]]);
        match value {
            0x09 | 0x0a | 0x0d | 0x20..=0x7e => ascii = true,
            0x3000..=0x30ff
            | 0x3400..=0x4dbf
            | 0x4e00..=0x9fff
            | 0xac00..=0xd7af
            | 0xff00..=0xffef => cjk = true,
            _ => return false,
        }
    }
    ascii || (cjk && bytes.iter().any(|&byte| byte >= 0x80))
}

fn append_pdf_string(
    output: &mut String,
    token: PdfToken<'_>,
    cmap: Option<&CMap>,
) -> AppResult<()> {
    let (bytes, hex) = match token {
        PdfToken::Literal(value) => (literal_bytes(value), false),
        PdfToken::Hex(value) => {
            let Some(bytes) = hex_bytes(value) else {
                return Ok(());
            };
            (bytes, true)
        }
        _ => return Ok(()),
    };
    if let Some(value) = bytes
        .strip_prefix(b"\xfe\xff")
        .or_else(|| (hex && cmap.is_none() && looks_utf16_be(&bytes)).then_some(bytes.as_slice()))
    {
        for ch in utf16_be(value) {
            append_pdf_char(output, ch)?;
        }
        return Ok(());
    }
    let mut rest = bytes.as_slice();
    while let Some((&byte, remaining)) = rest.split_first() {
        if let Some((width, range, mut delta)) = cmap.and_then(|cmap| cmap.lookup(rest)) {
            if delta == 0 {
                for ch in utf16_be(&range.target) {
                    append_pdf_char(output, ch)?;
                }
            } else {
                let mut target = range.target.clone();
                for byte in target.iter_mut().rev() {
                    let carry = u32::from(*byte) + (delta & 0xff);
                    *byte = carry as u8;
                    delta = (delta >> 8) + (carry >> 8);
                }
                if delta == 0 {
                    for ch in utf16_be(&target) {
                        append_pdf_char(output, ch)?;
                    }
                }
            }
            rest = &rest[width..];
        } else {
            append_pdf_char(output, pdf_doc_char(byte))?;
            rest = remaining;
        }
    }
    Ok(())
}

fn text_break(output: &mut String, separator: &str) -> AppResult<()> {
    if output.chars().next_back().is_some_and(|ch| {
        if separator == "\n" {
            ch != '\n'
        } else {
            !ch.is_whitespace()
        }
    }) {
        if separator == "\n" {
            output.truncate(output.trim_end_matches([' ', '\t']).len());
        }
        append(output, separator)?;
    }
    Ok(())
}

fn pdf_name(input: &[u8]) -> Cow<'_, [u8]> {
    if !input.contains(&b'#') {
        return Cow::Borrowed(input);
    }
    let mut decoded = Vec::new();
    let mut pos = 0;
    while let Some(&byte) = input.get(pos) {
        if byte == b'#' {
            if let Some((high, low)) = input
                .get(pos + 1)
                .and_then(|&b| hex_digit(b))
                .zip(input.get(pos + 2).and_then(|&b| hex_digit(b)))
            {
                decoded.push(high * 16 + low);
                pos += 3;
                continue;
            }
        }
        decoded.push(byte);
        pos += 1;
    }
    Cow::Owned(decoded)
}

fn pdf_text(
    bytes: &[u8],
    fonts: &HashMap<Cow<'_, [u8]>, Option<ObjectId>>,
    maps: &HashMap<ObjectId, CMap>,
    fallback: Option<&CMap>,
) -> AppResult<String> {
    let mut lexer = PdfLexer::new(bytes);
    let mut output = String::new();
    let mut in_text = false;
    let mut operands = [PdfToken::Other; 3];
    let mut array = None;
    let mut font = None;
    while let Some(token) = lexer.next() {
        match token {
            PdfToken::Word(b"BT") => {
                text_break(&mut output, "\n")?;
                in_text = true;
            }
            PdfToken::Word(b"ET") => in_text = false,
            PdfToken::Word(b"BI") if !in_text => {
                // Skip inline image data until a delimited EI marker.
                let rest = &bytes[lexer.pos..];
                if let Some(index) = rest.windows(4).position(|bytes| {
                    pdf_space(bytes[0]) && &bytes[1..3] == b"EI" && pdf_delimiter(bytes[3])
                }) {
                    lexer.pos += index + 3;
                } else {
                    break;
                }
            }
            _ if !in_text => continue,
            PdfToken::ArrayStart => {
                let start = lexer.pos;
                let mut depth = 1;
                while let Some(token) = lexer.next() {
                    match token {
                        PdfToken::ArrayStart => depth += 1,
                        PdfToken::ArrayEnd => {
                            depth -= 1;
                            if depth == 0 {
                                array = Some(&bytes[start..lexer.pos - 1]);
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                continue;
            }
            PdfToken::Name(_) | PdfToken::Literal(_) | PdfToken::Hex(_) => {
                operands.rotate_left(1);
                operands[2] = token;
                continue;
            }
            PdfToken::Word(_) if number(token).is_some() => {
                operands.rotate_left(1);
                operands[2] = token;
                continue;
            }
            PdfToken::Word(b"Tf") => {
                font = match operands[1] {
                    PdfToken::Name(name) => {
                        let name = pdf_name(name);
                        fonts
                            .get(name.as_ref())
                            .map(|id| id.and_then(|id| maps.get(&id)))
                    }
                    _ => None,
                };
            }
            PdfToken::Word(b"Tj" | b"'" | b"\"") => {
                if token != PdfToken::Word(b"Tj") {
                    text_break(&mut output, "\n")?;
                }
                append_pdf_string(&mut output, operands[2], font.unwrap_or(fallback))?;
            }
            PdfToken::Word(b"TJ") => {
                if let Some(array) = array {
                    let mut items = PdfLexer::new(array);
                    while let Some(item) = items.next() {
                        if number(item).is_some_and(|n| n < -180.0) {
                            text_break(&mut output, " ")?;
                        } else {
                            append_pdf_string(&mut output, item, font.unwrap_or(fallback))?;
                        }
                    }
                }
            }
            PdfToken::Word(b"T*") => text_break(&mut output, "\n")?,
            PdfToken::Word(b"Td" | b"TD") if number(operands[2]).is_some_and(|n| n != 0.0) => {
                text_break(&mut output, "\n")?;
            }
            _ => {}
        }
        array = None;
        operands.fill(PdfToken::Other);
    }
    output.truncate(output.trim_end().len());
    Ok(output)
}

const ENCRYPTED_PDF: &str = "该 PDF 已加密，无法提取文字，请先解密后再导入。";
const SCANNED_PDF: &str = "未在该 PDF 中检测到可提取的文字层（可能是纯扫描图片版 PDF）。你可以直接截图并按 Ctrl+V 粘贴到对话中让多模态模型识图分析。";

pub(crate) fn extract_pdf(bytes: &[u8]) -> AppResult<String> {
    let header = bytes[..bytes.len().min(1024)]
        .windows(5)
        .position(|window| window == b"%PDF-")
        .ok_or("无法识别 PDF 文件头。")?;
    let pdf = Pdf::parse(&bytes[header..])?;
    if pdf.encrypted {
        return Err(ENCRYPTED_PDF.into());
    }
    let mut budget = MAX_DOCUMENT_WORK;
    let cmap_ids: HashSet<_> = pdf
        .current_objects()
        .filter_map(|object| pdf_key(object.value, b"ToUnicode").and_then(pdf_ref))
        .collect();
    let mut maps = HashMap::new();
    let mut fallback = None;
    for object in pdf
        .current_objects()
        .filter(|object| cmap_ids.contains(&object.id) || pdf_type(object.value, b"CMap"))
    {
        if let Some(bytes) = decoded_stream(object, &mut budget)? {
            let map = CMap::parse(&bytes);
            if !map.ranges.is_empty() {
                fallback.get_or_insert(object.id);
                maps.insert(object.id, map);
            }
        }
    }
    let fallback = fallback.and_then(|id| maps.get(&id));
    let pages = pdf.pages();
    let mut output = String::new();
    let mut visited_contents = HashSet::new();
    let mut missing_contents = pages.is_empty();
    for (index, page) in pages.iter().enumerate() {
        let contents = pdf_key(page.value, b"Contents")
            .map(pdf_refs)
            .unwrap_or_default();
        let fonts = pdf.fonts(page);
        let mut text = String::new();
        let mut stack: Vec<_> = contents.into_iter().rev().collect();
        let mut expanded = HashSet::new();
        while let Some(id) = stack.pop() {
            let Some(object) = pdf.object(id) else {
                missing_contents = true;
                continue;
            };
            if object.stream.is_some() {
                visited_contents.insert(id);
                if let Some(bytes) = decoded_stream(object, &mut budget)? {
                    let content = pdf_text(&bytes, &fonts, &maps, fallback)?;
                    if !content.trim().is_empty() {
                        append_line(&mut text, &content)?;
                    }
                }
            } else if expanded.insert(id) {
                let refs = pdf_refs(object.value);
                missing_contents |= refs.is_empty();
                stack.extend(refs.into_iter().rev());
            }
        }
        append_pdf_page(&mut output, &text, index + 1, pages.len() > 1)?;
    }
    if output.is_empty() || missing_contents {
        let mut streams = Vec::new();
        let mut recovered_map = None;
        for object in pdf.current_objects().filter(|object| {
            object.stream.is_some()
                && !visited_contents.contains(&object.id)
                && !maps.contains_key(&object.id)
                && !pdf_type(object.value, b"ObjStm")
                && !pdf_type(object.value, b"XRef")
                && pdf_key(object.value, b"Subtype").and_then(|value| PdfLexer::new(value).next())
                    != Some(PdfToken::Name(b"Image"))
        }) {
            if let Some(bytes) = decoded_stream(object, &mut budget)? {
                // Font dictionaries inside object streams can leave ToUnicode streams unreferenced.
                if fallback.is_none()
                    && recovered_map.is_none()
                    && (bytes.windows(11).any(|part| part == b"beginbfchar")
                        || bytes.windows(12).any(|part| part == b"beginbfrange"))
                {
                    let map = CMap::parse(&bytes);
                    if !map.ranges.is_empty() {
                        recovered_map = Some(map);
                        continue;
                    }
                }
                if bytes.windows(2).any(|part| part == b"BT") {
                    streams.push(bytes);
                }
            }
        }
        let mut texts = Vec::new();
        let mut extracted = output.len();
        let fallback = fallback.or(recovered_map.as_ref());
        for bytes in streams {
            let text = pdf_text(&bytes, &HashMap::new(), &maps, fallback)?;
            if !text.trim().is_empty() {
                extracted += text.len();
                if extracted > MAX_TEXT_BYTES {
                    return Err(TEXT_LIMIT.into());
                }
                texts.push(text);
            }
        }
        let multiple = pages.len() > 1 || texts.len() > 1 || !output.is_empty();
        for (index, text) in texts.into_iter().enumerate() {
            append_pdf_page(&mut output, &text, pages.len() + index + 1, multiple)?;
        }
    }
    nonempty(output, SCANNED_PDF)
}

fn append_pdf_page(output: &mut String, text: &str, index: usize, numbered: bool) -> AppResult<()> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(());
    }
    if numbered {
        section(output, &format!("## 第 {index} 页\n"))?;
        append(output, text)
    } else {
        section(output, text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn package(parts: &[(&str, &str)]) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for &(name, xml) in parts {
            zip.start_file(name, options).unwrap();
            zip.write_all(xml.as_bytes()).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    fn stream(pdf: &mut Vec<u8>, id: u32, dictionary: &str, bytes: &[u8]) {
        write!(
            pdf,
            "{id} 0 obj\n<< /Length {} {dictionary} >>\nstream\n",
            bytes.len()
        )
        .unwrap();
        pdf.extend_from_slice(bytes);
        pdf.extend_from_slice(b"\nendstream\nendobj\n");
    }

    #[test]
    fn extracts_pptx_with_notes_xlsx_markdown_epub_chapters_and_compressed_pdf() {
        let slides = package(&[
            (
                "ppt/slides/slide10.xml",
                "<p:sld><a:p><a:t>十</a:t></a:p></p:sld>",
            ),
            (
                "ppt/slides/slide2.xml",
                "<p:sld><a:p><a:t>二</a:t></a:p></p:sld>",
            ),
            (
                "ppt/slides/slide1.xml",
                "<p:sld><a:p><a:t>一&#38;&#x6587;</a:t><a:br/><a:t>换行</a:t><a:tab/><a:t>制表</a:t></a:p></p:sld>",
            ),
            (
                "ppt/slides/_rels/slide1.xml.rels",
                r#"<Relationships><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide" Target="../notesSlides/notesSlide7.xml"/></Relationships>"#,
            ),
            (
                "ppt/notesSlides/notesSlide7.xml",
                r#"<p:notes><p:sp><p:ph type="sldNum"/><a:p><a:t>42</a:t></a:p></p:sp><p:sp><a:p><a:t>补充说明</a:t></a:p></p:sp></p:notes>"#,
            ),
        ]);
        assert_eq!(
            extract_pptx(&slides).unwrap(),
            "## 第 1 页\n一&文\n换行\t制表\n> 演讲者备注：补充说明\n\n## 第 2 页\n二\n\n## 第 10 页\n十"
        );
        let docx = package(&[(
            "word/document.xml",
            "<w:document><w:p><w:r><w:t>甲&#x4e59;&amp;amp;</w:t></w:r></w:p></w:document>",
        )]);
        assert_eq!(extract_docx(&docx).unwrap(), "甲乙&amp;");

        let workbook = package(&[
            (
                "xl/workbook.xml",
                r#"<workbook><sheets><sheet name="统计" r:id="rId8"/></sheets></workbook>"#,
            ),
            (
                "xl/_rels/workbook.xml.rels",
                r#"<Relationships><Relationship Id="rId8" Target="/xl/worksheets/data.xml"/></Relationships>"#,
            ),
            (
                "xl/sharedStrings.xml",
                "<sst><si><t>名</t></si><si><r><t>数</t></r><r><t>量</t></r></si></sst>",
            ),
            (
                "xl/worksheets/data.xml",
                r#"<worksheet><sheetData>
                <row><c r="A1" t="s"><v>0</v></c><c r="C1" t="s"><v>1</v></c></row>
                <row><c r="A2" t="inlineStr"><is><t>甲|乙&#10;丙</t></is></c><c r="C2"><f>6*7</f><v>42</v></c><c r="ZZ2"><v>1000</v></c></row>
            </sheetData></worksheet>"#,
            ),
        ]);
        assert_eq!(
            extract_xlsx(&workbook).unwrap(),
            "## 统计\n| 名 |  | 数量 |\n| --- | --- | --- |\n| 甲\\|乙<br>丙 |  | 42 |"
        );

        let book = package(&[
            (
                "META-INF/container.xml",
                r#"<container><rootfiles><rootfile full-path="OEBPS/content.opf"/></rootfiles></container>"#,
            ),
            (
                "OEBPS/content.opf",
                r#"<package><manifest>
                <item id="a" href="a.xhtml"/><item id="z" href="./z.xhtml"/>
            </manifest><spine><itemref idref="z"/><itemref idref="a"/></spine></package>"#,
            ),
            (
                "OEBPS/a.xhtml",
                "<html><body><h2>第二章</h2><div><p>结语</p></div></body></html>",
            ),
            (
                "OEBPS/z.xhtml",
                "<html><head><title>元数据</title></head><body><h1>第一章</h1><script>if (a < b) hide()</script><style>p {color:red}</style><p>你好 <em>世界</em>&nbsp;！</p></body></html>",
            ),
        ]);
        assert_eq!(
            extract_epub(&book).unwrap(),
            "## 第一章\n\n你好 世界 ！\n\n## 第二章\n\n结语"
        );

        let mut pdf = br"%PDF-1.7
1 0 obj
<< /Type /Catalog /Pages 2 0 R >>
endobj
2 0 obj
<< /Type /Pages /Kids [4 0 R 3 0 R] /Count 2 /Resources 8 0 R >>
endobj
3 0 obj
<< /Type /Page /Parent 2 0 R /Contents [5 0 R] >>
endobj
4 0 obj
<< /Type /Page /Parent 2 0 R /Contents 6 0 R >>
endobj
7 0 obj
<< /Type /Font /ToUnicode 10 0 R >>
endobj
8 0 obj
<< /Font 9 0 R >>
endobj
9 0 obj
<< /F1 7 0 R /English << /Type /Font >> >>
endobj
"
        .to_vec();
        stream(&mut pdf, 5, "", br#"BT /English 12 Tf [(English) -210 (text\041)] TJ (Next) ' 0 0 (\(literal\)\\) " T* <FEFF4E2D6587> Tj ET"#);
        let compressed = hex_bytes(
            b"789c730a51d0773354303452084953b03130303004622320360662133b85902c05d7100093f707f5",
        )
        .unwrap();
        stream(&mut pdf, 6, "/Filter /FlateDecode", &compressed);
        stream(
            &mut pdf,
            10,
            "",
            br"
            1 beginbfchar
            <0001> <8BFE>
            endbfchar
            1 beginbfrange
            <0002> <0002> <4EF6>
            endbfrange
            1 beginbfrange
            <0003> <0004> [<6D4B> <8BD5>]
            endbfrange
        ",
        );
        pdf.extend_from_slice(b"trailer\n<< /Root 1 0 R >>\n%%EOF\n");
        assert_eq!(
            extract_pdf(&pdf).unwrap(),
            "## 第 1 页\n课件测试\n\n## 第 2 页\nEnglish text!\nNext\n(literal)\\\n中文"
        );
        assert_eq!(extract_pdf(b"%PDF-1.7\n%%EOF").unwrap_err(), SCANNED_PDF);
        assert_eq!(
            extract_pdf(b"%PDF-1.7\ntrailer\n<< /Encrypt 5 0 R >>\n%%EOF").unwrap_err(),
            ENCRYPTED_PDF
        );
        let mut fallback_pdf = b"junk\n%PDF-1.5\n".to_vec();
        stream(&mut fallback_pdf, 20, "/Filter [/FlateDecode]", &compressed);
        stream(
            &mut fallback_pdf,
            21,
            "",
            b"1 beginbfrange\n<0001> <0004> <4E00>\nendbfrange",
        );
        assert_eq!(extract_pdf(&fallback_pdf).unwrap(), "一丁丂七");
    }

    #[test]
    fn inflate_reads_stored_fixed_and_dynamic_blocks() {
        let cases = [
            (
                "7801010c00f3ff73746f72656420626c6f636b1f8004bd",
                "stored block".to_owned(),
            ),
            (
                "78014bcbac484d51f0284d4bcb4dcc534843e601008c370a0b",
                "fixed Huffman fixed Huffman".to_owned(),
            ),
            (
                "78daedc8c10d80301003c1565c013d19e5501e885884f44f1d887dcef84cf75e8fe658911142e84bba9aace6d9350ea592ba37966559f617fb023dd6d0b8",
                "alphabet soup ".repeat(87) + &"and a dash of pepper. ".repeat(70),
            ),
        ];
        for (block_type, (hex, expected)) in cases.into_iter().enumerate() {
            let mut data = hex_bytes(hex.as_bytes()).unwrap();
            assert_eq!(((data[2] >> 1) & 3) as usize, block_type);
            assert_eq!(inflate_zlib(&data).unwrap(), expected.as_bytes());
            *data.last_mut().unwrap() ^= 0xff;
            assert!(inflate_zlib(&data).is_none());
            assert!(inflate_zlib(&data[..6]).is_none());
        }
        assert!(inflate_zlib(b"").is_none());
    }

    #[test]
    fn rejects_oversized_xml_and_extracted_text() {
        let xml = " ".repeat(MAX_XML_BYTES as usize + 1);
        let docx = package(&[("word/document.xml", &xml)]);
        assert!(
            extract_docx(&docx)
                .unwrap_err()
                .contains("解压后的内容过大")
        );

        let text = "文".repeat(MAX_TEXT_BYTES / 3);
        let slide = format!("<p:sld><a:p><a:t>{text}</a:t></a:p></p:sld>");
        let slides = package(&[("ppt/slides/slide1.xml", &slide)]);
        assert_eq!(extract_pptx(&slides).unwrap_err(), TEXT_LIMIT);
    }
}
