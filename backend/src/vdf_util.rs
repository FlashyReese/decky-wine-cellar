//! Read-only deserialization of binary KeyValues v1 into a reusable value tree.
//!
//! Keys and narrow strings retain their original bytes, floats retain their bit
//! patterns, and objects retain entry order, duplicate keys and end markers.
//! File-specific headers or containers must be decoded before calling this parser.
use std::io::{self, ErrorKind};

pub const MAX_FILE_BYTES: usize = 16 * 1024 * 1024;
const MAX_DEPTH: usize = 64;
const MAX_ENTRIES: usize = 100_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry<'a> {
    pub key: &'a [u8],
    pub value: Value<'a>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndMarker {
    End,
    EndAlt,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Object<'a> {
    pub entries: Vec<Entry<'a>>,
    pub end: EndMarker,
}

impl<'a> Object<'a> {
    /// Find a single field ignoring ASCII case; ambiguous fields return `None`.
    pub fn get_unique(&self, key: &[u8]) -> Option<&Value<'a>> {
        let mut matches = self
            .entries
            .iter()
            .filter(|entry| entry.key.eq_ignore_ascii_case(key));
        let value = &matches.next()?.value;
        matches.next().is_none().then_some(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value<'a> {
    Object(Object<'a>),
    String(&'a [u8]),
    Int32(i32),
    /// Use `f32::from_bits` to inspect the numeric value without losing NaN bits.
    Float32Bits(u32),
    Pointer(u32),
    WideString(Vec<u16>),
    Color([u8; 4]),
    UInt64(u64),
    Int64(i64),
}

struct Reader<'a> {
    remaining: &'a [u8],
    entries: usize,
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(ErrorKind::InvalidData, message)
}

impl<'a> Reader<'a> {
    fn take(&mut self, length: usize) -> io::Result<&'a [u8]> {
        if self.remaining.len() < length {
            return Err(invalid("Truncated binary VDF"));
        }
        let (value, remaining) = self.remaining.split_at(length);
        self.remaining = remaining;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> io::Result<[u8; N]> {
        let mut value = [0; N];
        value.copy_from_slice(self.take(N)?);
        Ok(value)
    }

    fn string(&mut self, wide: bool) -> io::Result<&'a [u8]> {
        let width = if wide { 2 } else { 1 };
        let length = self
            .remaining
            .chunks_exact(width)
            .position(|unit| unit.iter().all(|byte| *byte == 0))
            .ok_or_else(|| invalid("Unterminated binary VDF string"))?
            * width;
        let value = self.take(length)?;
        self.take(width)?;
        Ok(value)
    }

    fn object(&mut self, depth: usize) -> io::Result<Object<'a>> {
        if depth > MAX_DEPTH {
            return Err(invalid("Binary VDF nesting limit exceeded"));
        }
        let mut entries = Vec::new();
        loop {
            let tag = self.take(1)?[0];
            let end = match tag {
                8 => Some(EndMarker::End),
                11 => Some(EndMarker::EndAlt),
                _ => None,
            };
            if let Some(end) = end {
                return Ok(Object { entries, end });
            }
            self.entries += 1;
            if self.entries > MAX_ENTRIES {
                return Err(invalid("Binary VDF entry limit exceeded"));
            }
            let key = self.string(false)?;
            let value = match tag {
                0 => Value::Object(self.object(depth + 1)?),
                1 => Value::String(self.string(false)?),
                2 => Value::Int32(i32::from_le_bytes(self.array()?)),
                3 => Value::Float32Bits(u32::from_le_bytes(self.array()?)),
                4 => Value::Pointer(u32::from_le_bytes(self.array()?)),
                5 => Value::WideString(
                    self.string(true)?
                        .chunks_exact(2)
                        .map(|unit| u16::from_le_bytes([unit[0], unit[1]]))
                        .collect(),
                ),
                6 => Value::Color(self.array()?),
                7 => Value::UInt64(u64::from_le_bytes(self.array()?)),
                10 => Value::Int64(i64::from_le_bytes(self.array()?)),
                _ => return Err(invalid(&format!("Unsupported binary VDF type: {tag}"))),
            };
            entries.push(Entry { key, value });
        }
    }
}

/// Parse a complete binary KeyValues v1 document, independent of its file schema.
///
/// The returned tree borrows keys and narrow strings from `data`. Input is
/// bounded to 16 MiB, 64 nested objects and 100,000 entries; unsupported tags,
/// truncated values and trailing bytes return an error.
pub fn parse_binary(data: &[u8]) -> io::Result<Object<'_>> {
    if data.len() > MAX_FILE_BYTES {
        return Err(invalid("Binary VDF file exceeds size limit"));
    }
    let mut reader = Reader {
        remaining: data,
        entries: 0,
    };
    let root = reader.object(0)?;
    if !reader.remaining.is_empty() {
        return Err(invalid("Unexpected data after binary VDF root"));
    }
    Ok(root)
}
