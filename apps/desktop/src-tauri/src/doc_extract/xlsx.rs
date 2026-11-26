use super::{Package, append, attr, nonempty, package_path, section, xml_events, xml_text};
use crate::models::AppResult;
use quick_xml::events::Event;
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
};

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
