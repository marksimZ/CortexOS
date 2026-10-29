//! Exact JSON decoding, local Schema validation and typed semantic comparison.
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::{Value, value::RawValue};
use std::{collections::BTreeSet, fmt};

struct Object(Vec<Box<RawValue>>);
impl<'de> Deserialize<'de> for Object {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Fields;
        impl<'de> Visitor<'de> for Fields {
            type Value = Object;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an object with unique keys")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut m: M) -> Result<Object, M::Error> {
                let mut seen = BTreeSet::new();
                let mut values = Vec::new();
                while let Some(key) = m.next_key::<String>()? {
                    if !seen.insert(key.clone()) || key.starts_with("$serde_json::private::") {
                        return Err(serde::de::Error::custom("duplicate_or_reserved_key"));
                    }
                    let raw = m.next_value::<Box<RawValue>>()?;
                    values.push(raw);
                }
                Ok(Object(values))
            }
        }
        d.deserialize_map(Fields)
    }
}
fn inspect(raw: &RawValue, depth: usize) -> Result<(), String> {
    if depth > 64 {
        return Err("json_depth_limit".into());
    }
    match raw.get().trim_start().as_bytes().first() {
        Some(b'{') => {
            let object = serde_json::from_str::<Object>(raw.get()).map_err(|e| e.to_string())?;
            for value in object.0 {
                inspect(&value, depth + 1)?;
            }
        }
        Some(b'[') => {
            let values: Vec<Box<RawValue>> =
                serde_json::from_str(raw.get()).map_err(|e| e.to_string())?;
            for v in values {
                inspect(&v, depth + 1)?;
            }
        }
        _ => (),
    }
    Ok(())
}
pub fn decode(text: &str) -> Result<Value, String> {
    if text.len() > 1024 * 1024 {
        return Err("json_output_limit".into());
    }
    let raw: Box<RawValue> = serde_json::from_str(text).map_err(|e| e.to_string())?;
    inspect(&raw, 0)?;
    serde_json::from_str(raw.get()).map_err(|e| e.to_string())
}

// Canonical decimal coefficient/exponent: never compare JSON numbers through f64.
fn decimal(n: &serde_json::Number) -> Option<(bool, String, i64)> {
    let s = n.to_string();
    let (negative, s) = s
        .strip_prefix('-')
        .map_or((false, s.as_str()), |s| (true, s));
    let (mantissa, exponent) = s
        .split_once(['e', 'E'])
        .map_or(Some((s, 0)), |(a, b)| b.parse::<i64>().ok().map(|b| (a, b)))?;
    let fractional = mantissa.split_once('.').map_or(0, |(_, b)| b.len() as i64);
    let mut digits = mantissa
        .replace('.', "")
        .trim_start_matches('0')
        .to_string();
    if digits.is_empty() {
        return Some((false, "0".into(), 0));
    }
    let mut exp = exponent.checked_sub(fractional)?;
    while digits.ends_with('0') {
        digits.pop();
        exp = exp.checked_add(1)?;
    }
    Some((negative, digits, exp))
}
pub fn equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => {
            decimal(a).zip(decimal(b)).is_some_and(|(a, b)| a == b)
        }
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| equal(a, b))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len() && a.iter().all(|(k, a)| b.get(k).is_some_and(|b| equal(a, b)))
        }
        _ => a == b,
    }
}
pub fn unsigned(value: &Value) -> Option<u64> {
    let (negative, digits, exponent) = decimal(value.as_number()?)?;
    if negative || !(0..=19).contains(&exponent) {
        return None;
    }
    digits
        .parse::<u64>()
        .ok()?
        .checked_mul(10u64.checked_pow(exponent as u32)?)
}
fn vocabulary(schema: &Value) -> Result<(), String> {
    let object = schema.as_object().ok_or("schema_not_object")?;
    for key in object.keys() {
        if ![
            "$schema",
            "type",
            "properties",
            "required",
            "additionalProperties",
            "items",
            "minItems",
            "maxItems",
            "minimum",
            "maximum",
            "minLength",
            "maxLength",
            "enum",
            "format",
        ]
        .contains(&key.as_str())
        {
            return Err(format!("unsupported_schema_keyword:{key}"));
        }
    }
    if schema
        .get("$schema")
        .is_some_and(|s| s != "https://json-schema.org/draft/2020-12/schema")
    {
        return Err("unsupported_schema_dialect".into());
    }
    if let Some(properties) = schema.get("properties") {
        for rule in properties.as_object().ok_or("invalid_properties")?.values() {
            vocabulary(rule)?;
        }
    }
    if let Some(items) = schema.get("items") {
        vocabulary(items)?;
    }
    if schema.get("format").is_some_and(|v| v != "date") {
        return Err("unsupported_format".into());
    }
    Ok(())
}
pub fn schema_valid(schema: &Value, value: &Value) -> Result<bool, String> {
    vocabulary(schema)?;
    let validator = jsonschema::draft202012::options()
        .should_validate_formats(true)
        .build(schema)
        .map_err(|e| format!("invalid_schema:{e}"))?;
    Ok(validator.is_valid(value))
}

/// Ollama sends complete argument objects. Indexed frames can extend an object;
/// unindexed entries are separate calls, including duplicate invocations.
pub fn assemble_calls(frames: &[Vec<Value>]) -> Result<Vec<Value>, String> {
    let mut calls: Vec<Value> = Vec::new();
    let mut indexes = std::collections::BTreeMap::new();
    for frame in frames {
        let mut frame_indexes = BTreeSet::new();
        for call in frame {
            let index = call["index"]
                .as_u64()
                .or_else(|| call["function"]["index"].as_u64());
            if let Some(index) = index {
                if !frame_indexes.insert(index) {
                    return Err("duplicate_tool_index_in_frame".into());
                }
                if let Some(&position) = indexes.get(&index) {
                    let saved: &mut Value = &mut calls[position];
                    if call["function"]["name"] != saved["function"]["name"] {
                        return Err("tool_fragment_name_changed".into());
                    }
                    let incoming = call["function"]["arguments"]
                        .as_object()
                        .ok_or("invalid_tool_fragment")?;
                    let args = saved["function"]["arguments"]
                        .as_object_mut()
                        .ok_or("invalid_tool_fragment")?;
                    for (k, v) in incoming {
                        if args.get(k).is_some_and(|old| !equal(old, v)) {
                            return Err("tool_fragment_argument_conflict".into());
                        }
                        args.insert(k.clone(), v.clone());
                    }
                    continue;
                }
                indexes.insert(index, calls.len());
            }
            calls.push(call.clone());
            if calls.len() > 32 {
                return Err("tool_call_limit".into());
            }
        }
    }
    Ok(calls)
}
