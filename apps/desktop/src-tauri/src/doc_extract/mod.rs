mod docx;
mod epub;
mod pdf;
mod pptx;
mod xlsx;

pub(crate) use docx::extract_docx;
pub(crate) use epub::extract_epub;
pub(crate) use pdf::extract_pdf;
pub(crate) use pptx::extract_pptx;
pub(crate) use xlsx::extract_xlsx;

use crate::models::AppResult;
use quick_xml::{
    Reader, XmlVersion,
    events::{BytesStart, Event},
};
use std::{
    borrow::Cow,
    io::{Cursor, Read},
};

const MAX_TEXT_BYTES: usize = 2 * 1024 * 1024;
const MAX_XML_BYTES: u64 = 4 * 1024 * 1024;
const MAX_DOCUMENT_WORK: usize = 64 * 1024 * 1024;
const XML_ERROR: &str = "文档 XML 格式或编码异常。";
const TEXT_LIMIT: &str = "提取的文字超过 2 MiB，请拆分文档后重试。";

pub(crate) const MAX_BINARY_FILE_BYTES: u64 = 32 * 1024 * 1024;

pub(crate) fn extract_document_text(extension: &str, bytes: &[u8]) -> Option<AppResult<String>> {
    let extract = match extension {
        "docx" => extract_docx,
        "pptx" => extract_pptx,
        "xlsx" => extract_xlsx,
        "epub" => extract_epub,
        "pdf" => extract_pdf,
        _ => return None,
    };
    Some(extract(bytes))
}

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

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::pdf::{ENCRYPTED_PDF, SCANNED_PDF, hex_bytes, inflate_zlib};
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
