//! Wire frame of the engine ABI v1 (ADR 0035): request and response between a host and an
//! `engine.wasm`.
//!
//! Layout: `u32` little-endian JSON length, the JSON (UTF-8), then the binary blobs back to back.
//! The JSON keeps the serde shape of [`Value`] (`{"Text":"x"}`, `{"Int":3}`, …) except that the
//! bytes of a file are not in the JSON: `{"File":{"name","mime","blob":i}}` points at blob `i`,
//! and the top-level `"blobs"` array lists the blob lengths. Large files therefore never pass
//! through a JSON number list.
use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

use serde_json::{Map, Number, Value as Json, json};

use crate::{FileData, Table, ToolError, Value, ValueMap};

/// Error answered by an engine (or by the frame itself): `code` is translated by the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub code: String,
    pub detail: Option<String>,
}

impl Failure {
    fn protocol(detail: impl Into<String>) -> Self {
        Self {
            code: "worker_protocol".to_owned(),
            detail: Some(detail.into()),
        }
    }

    /// The same error with a `'static` code (the interned string), as [`ToolError`] needs.
    #[must_use]
    pub fn into_tool_error(self) -> ToolError {
        ToolError {
            code: static_code(&self.code),
            detail: self.detail,
        }
    }
}

/// Returns a `'static` copy of `code`; each distinct code is leaked once.
#[must_use]
pub fn static_code(code: &str) -> &'static str {
    static CODES: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    let mut codes = CODES
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(known) = codes.get(code) {
        return known;
    }
    let leaked: &'static str = Box::leak(code.to_owned().into_boxed_str());
    codes.insert(leaked);
    leaked
}

fn number(f: f64) -> Json {
    Number::from_f64(f).map_or_else(
        || {
            Json::String(
                if f.is_nan() {
                    "NaN"
                } else if f > 0.0 {
                    "Inf"
                } else {
                    "-Inf"
                }
                .to_owned(),
            )
        },
        Json::Number,
    )
}

fn file<'a>(f: &'a FileData, blobs: &mut Vec<&'a [u8]>) -> Json {
    blobs.push(&f.bytes);
    json!({"name": f.name, "mime": f.mime, "blob": blobs.len() - 1})
}

fn value<'a>(v: &'a Value, blobs: &mut Vec<&'a [u8]>) -> Json {
    match v {
        Value::Text(s) => json!({"Text": s}),
        Value::Number(f) => json!({"Number": number(*f)}),
        Value::Int(i) => json!({"Int": i}),
        Value::Bool(b) => json!({"Bool": b}),
        Value::File(f) => json!({"File": file(f, blobs)}),
        Value::Files(list) => {
            json!({"Files": list.iter().map(|f| file(f, blobs)).collect::<Vec<_>>()})
        }
        Value::Table(t) => json!({"Table": {"columns": t.columns, "rows": t.rows}}),
    }
}

fn map<'a>(m: &'a ValueMap, blobs: &mut Vec<&'a [u8]>) -> Json {
    Json::Object(
        m.iter()
            .map(|(k, v)| (k.clone(), value(v, blobs)))
            .collect::<Map<_, _>>(),
    )
}

fn pack(mut head: Map<String, Json>, blobs: &[&[u8]]) -> Vec<u8> {
    head.insert(
        "blobs".to_owned(),
        Json::Array(blobs.iter().map(|b| json!(b.len())).collect()),
    );
    let text = Json::Object(head).to_string();
    let mut out = Vec::with_capacity(4 + text.len() + blobs.iter().map(|b| b.len()).sum::<usize>());
    out.extend_from_slice(&u32::try_from(text.len()).unwrap_or(u32::MAX).to_le_bytes());
    out.extend_from_slice(text.as_bytes());
    for blob in blobs {
        out.extend_from_slice(blob);
    }
    out
}

/// Request frame for `kx_run`.
#[must_use]
pub fn encode_request(inputs: &ValueMap, params: &ValueMap) -> Vec<u8> {
    let mut blobs = Vec::new();
    let mut head = Map::new();
    head.insert("inputs".to_owned(), map(inputs, &mut blobs));
    head.insert("params".to_owned(), map(params, &mut blobs));
    pack(head, &blobs)
}

/// Response frame of `kx_run`.
#[must_use]
pub fn encode_response(outcome: &Result<ValueMap, ToolError>) -> Vec<u8> {
    let mut blobs = Vec::new();
    let mut head = Map::new();
    match outcome {
        Ok(outputs) => {
            head.insert("Ok".to_owned(), map(outputs, &mut blobs));
        }
        Err(error) => {
            head.insert(
                "Err".to_owned(),
                json!({"code": error.code, "detail": error.detail}),
            );
        }
    }
    pack(head, &blobs)
}

fn split(bytes: &[u8]) -> Result<(Json, Vec<&[u8]>), Failure> {
    let head = bytes
        .get(..4)
        .ok_or_else(|| Failure::protocol("frame too short"))?;
    let json_len = u32::from_le_bytes([head[0], head[1], head[2], head[3]]) as usize;
    let json_end = 4_usize
        .checked_add(json_len)
        .filter(|end| *end <= bytes.len())
        .ok_or_else(|| Failure::protocol("frame JSON truncated"))?;
    let json: Json = serde_json::from_slice(&bytes[4..json_end])
        .map_err(|e| Failure::protocol(format!("frame JSON: {e}")))?;
    let lengths = json
        .get("blobs")
        .and_then(Json::as_array)
        .ok_or_else(|| Failure::protocol("frame without blob list"))?;
    let mut blobs = Vec::with_capacity(lengths.len());
    let mut pos = json_end;
    for length in lengths {
        let length = length
            .as_u64()
            .and_then(|l| usize::try_from(l).ok())
            .ok_or_else(|| Failure::protocol("invalid blob length"))?;
        let end = pos
            .checked_add(length)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| Failure::protocol("blob truncated"))?;
        blobs.push(&bytes[pos..end]);
        pos = end;
    }
    if pos != bytes.len() {
        return Err(Failure::protocol("bytes after the last blob"));
    }
    Ok((json, blobs))
}

fn text(v: &Json, what: &str) -> Result<String, Failure> {
    v.as_str()
        .map(str::to_owned)
        .ok_or_else(|| Failure::protocol(format!("{what}: expected a string")))
}

fn decode_file(v: &Json, blobs: &[&[u8]]) -> Result<FileData, Failure> {
    let index = v
        .get("blob")
        .and_then(Json::as_u64)
        .and_then(|i| usize::try_from(i).ok())
        .ok_or_else(|| Failure::protocol("file without blob index"))?;
    let bytes = blobs
        .get(index)
        .ok_or_else(|| Failure::protocol("blob index out of range"))?;
    Ok(FileData {
        name: text(v.get("name").unwrap_or(&Json::Null), "file name")?,
        mime: text(v.get("mime").unwrap_or(&Json::Null), "file mime")?,
        bytes: bytes.to_vec(),
    })
}

fn strings(v: &Json, what: &str) -> Result<Vec<String>, Failure> {
    v.as_array()
        .ok_or_else(|| Failure::protocol(format!("{what}: expected a list")))?
        .iter()
        .map(|s| text(s, what))
        .collect()
}

fn decode_value(v: &Json, blobs: &[&[u8]]) -> Result<Value, Failure> {
    let object = v
        .as_object()
        .filter(|o| o.len() == 1)
        .ok_or_else(|| Failure::protocol("value must be a one-key object"))?;
    let (tag, inner) = object
        .iter()
        .next()
        .ok_or_else(|| Failure::protocol("empty value"))?;
    match tag.as_str() {
        "Text" => Ok(Value::Text(text(inner, "Text")?)),
        "Number" => match inner {
            Json::Number(n) => n
                .as_f64()
                .map(Value::Number)
                .ok_or_else(|| Failure::protocol("Number out of range")),
            Json::String(s) => match s.as_str() {
                "NaN" => Ok(Value::Number(f64::NAN)),
                "Inf" => Ok(Value::Number(f64::INFINITY)),
                "-Inf" => Ok(Value::Number(f64::NEG_INFINITY)),
                _ => Err(Failure::protocol("Number: unknown literal")),
            },
            _ => Err(Failure::protocol("Number: expected a number")),
        },
        "Int" => inner
            .as_i64()
            .map(Value::Int)
            .ok_or_else(|| Failure::protocol("Int: expected an integer")),
        "Bool" => inner
            .as_bool()
            .map(Value::Bool)
            .ok_or_else(|| Failure::protocol("Bool: expected a boolean")),
        "File" => Ok(Value::File(decode_file(inner, blobs)?)),
        "Files" => inner
            .as_array()
            .ok_or_else(|| Failure::protocol("Files: expected a list"))?
            .iter()
            .map(|f| decode_file(f, blobs))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Files),
        "Table" => {
            let columns = strings(inner.get("columns").unwrap_or(&Json::Null), "Table columns")?;
            let rows = inner
                .get("rows")
                .and_then(Json::as_array)
                .ok_or_else(|| Failure::protocol("Table: expected rows"))?
                .iter()
                .map(|r| strings(r, "Table row"))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Value::Table(Table { columns, rows }))
        }
        other => Err(Failure::protocol(format!("unknown value tag {other}"))),
    }
}

fn decode_map(v: &Json, blobs: &[&[u8]]) -> Result<ValueMap, Failure> {
    match v {
        Json::Null => Ok(ValueMap::new()),
        Json::Object(entries) => entries
            .iter()
            .map(|(k, v)| decode_value(v, blobs).map(|v| (k.clone(), v)))
            .collect(),
        _ => Err(Failure::protocol("expected an object of values")),
    }
}

/// Decodes a request frame into `(inputs, params)`.
///
/// # Errors
/// `worker_protocol` when the frame is malformed.
pub fn decode_request(bytes: &[u8]) -> Result<(ValueMap, ValueMap), Failure> {
    let (json, blobs) = split(bytes)?;
    Ok((
        decode_map(json.get("inputs").unwrap_or(&Json::Null), &blobs)?,
        decode_map(json.get("params").unwrap_or(&Json::Null), &blobs)?,
    ))
}

/// Decodes a response frame into the outputs, or the engine's failure.
///
/// # Errors
/// The engine's own error, or `worker_protocol` when the frame is malformed.
pub fn decode_response(bytes: &[u8]) -> Result<ValueMap, Failure> {
    let (json, blobs) = split(bytes)?;
    if let Some(error) = json.get("Err") {
        return Err(Failure {
            code: text(error.get("code").unwrap_or(&Json::Null), "error code")?,
            detail: error
                .get("detail")
                .and_then(Json::as_str)
                .map(str::to_owned),
        });
    }
    decode_map(
        json.get("Ok")
            .ok_or_else(|| Failure::protocol("response is neither Ok nor Err"))?,
        &blobs,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ValueMap {
        ValueMap::from([
            ("t".to_owned(), Value::Text("héllo \"x\"".to_owned())),
            ("n".to_owned(), Value::Number(0.1 + 0.2)),
            ("i".to_owned(), Value::Int(i64::MIN)),
            ("b".to_owned(), Value::Bool(true)),
            (
                "f".to_owned(),
                Value::File(FileData {
                    name: "a.bin".to_owned(),
                    mime: "application/octet-stream".to_owned(),
                    bytes: (0..=255).collect(),
                }),
            ),
            (
                "fs".to_owned(),
                Value::Files(vec![
                    FileData {
                        name: "x".to_owned(),
                        mime: "text/plain".to_owned(),
                        bytes: vec![],
                    },
                    FileData {
                        name: "y".to_owned(),
                        mime: "text/plain".to_owned(),
                        bytes: b"yy".to_vec(),
                    },
                ]),
            ),
            (
                "tb".to_owned(),
                Value::Table(Table {
                    columns: vec!["a".to_owned(), "b".to_owned()],
                    rows: vec![vec!["1".to_owned(), "2".to_owned()]],
                }),
            ),
        ])
    }

    #[test]
    fn request_round_trips_every_value_kind() {
        let params = ValueMap::from([("p".to_owned(), Value::Int(3))]);
        let frame = encode_request(&sample(), &params);
        assert_eq!(decode_request(&frame), Ok((sample(), params)));
    }

    #[test]
    fn file_bytes_stay_out_of_the_json() {
        let frame = encode_request(&sample(), &ValueMap::new());
        let json_len = u32::from_le_bytes([frame[0], frame[1], frame[2], frame[3]]) as usize;
        let json = std::str::from_utf8(&frame[4..4 + json_len]).unwrap_or_default();
        assert!(json.contains("\"blob\":0"));
        assert!(
            !json.contains("255"),
            "bytes must not be listed in the JSON: {json}"
        );
        assert_eq!(frame.len(), 4 + json_len + 256 + 2);
    }

    #[test]
    fn non_finite_numbers_survive() {
        let outputs = ValueMap::from([
            ("a".to_owned(), Value::Number(f64::INFINITY)),
            ("b".to_owned(), Value::Number(f64::NEG_INFINITY)),
        ]);
        let decoded = decode_response(&encode_response(&Ok(outputs.clone())));
        assert_eq!(decoded, Ok(outputs));
        let nan = decode_response(&encode_response(&Ok(ValueMap::from([(
            "n".to_owned(),
            Value::Number(f64::NAN),
        )]))));
        assert!(matches!(
            nan.as_ref().ok().and_then(|m| m.get("n")),
            Some(Value::Number(n)) if n.is_nan()
        ));
    }

    #[test]
    fn response_carries_outputs_or_the_typed_error() {
        assert_eq!(
            decode_response(&encode_response(&Ok(sample()))),
            Ok(sample())
        );
        let failed = decode_response(&encode_response(&Err(ToolError::with_detail(
            "empty_input",
            "no text",
        ))));
        assert_eq!(
            failed,
            Err(Failure {
                code: "empty_input".to_owned(),
                detail: Some("no text".to_owned())
            })
        );
        assert_eq!(
            failed.map_err(Failure::into_tool_error),
            Err(ToolError::with_detail("empty_input", "no text"))
        );
    }

    #[test]
    fn malformed_frames_are_protocol_errors_never_panics() {
        let good = encode_request(&sample(), &ValueMap::new());
        for bad in [
            &[][..],
            &[1, 2][..],
            &[255, 255, 255, 255][..],
            &good[..good.len() - 1],
            &[good.clone(), vec![0]].concat()[..],
        ] {
            assert_eq!(
                decode_request(bad).map_err(|e| e.code),
                Err("worker_protocol".to_owned())
            );
        }
        let mut unknown = br#"{"inputs":{"a":{"Nope":1}},"blobs":[]}"#.to_vec();
        let mut frame = (unknown.len() as u32).to_le_bytes().to_vec();
        frame.append(&mut unknown);
        assert!(decode_request(&frame).is_err());
    }

    #[test]
    fn static_codes_are_interned() {
        assert!(std::ptr::eq(
            static_code("some_code"),
            static_code("some_code")
        ));
    }
}
