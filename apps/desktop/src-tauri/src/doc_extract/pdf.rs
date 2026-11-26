use super::{
    MAX_DOCUMENT_WORK, MAX_TEXT_BYTES, TEXT_LIMIT, append, append_line, hex_digit, nonempty,
    section,
};
use crate::models::AppResult;
use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap, HashSet},
};

const MAX_INFLATED_BYTES: usize = 16 * 1024 * 1024;

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

pub(super) fn inflate_zlib(data: &[u8]) -> Option<Vec<u8>> {
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

pub(super) fn hex_bytes(input: &[u8]) -> Option<Vec<u8>> {
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

pub(super) const ENCRYPTED_PDF: &str = "该 PDF 已加密，无法提取文字，请先解密后再导入。";
pub(super) const SCANNED_PDF: &str = "未在该 PDF 中检测到可提取的文字层（可能是纯扫描图片版 PDF）。你可以直接截图并按 Ctrl+V 粘贴到对话中让多模态模型识图分析。";

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
