//! 格式迁移：读旧文件时在这里显式补字段，补齐后交给当前格式解析。
//!
//! 迁移只在内存里做，不碰磁盘：读旧写新，保存时才落成当前格式。
//! 迁移这一步不写文件，所以失败也不会覆盖原文件。
//!
//! 补字段不删字段：未知字段原样留在映射里，解析时收进 extras，写回时照抄。

use crate::cbor::Value;
use crate::error::Error;
use crate::format::{self, FORMAT};

/// 把画板值升到当前格式；已经是当前格式的原样返回。
pub(crate) fn board(value: &Value) -> Result<Value, Error> {
    let found = format::format_of(value)?;
    if found >= FORMAT {
        return Ok(value.clone());
    }
    // 格式 1 → 2：画板补 tags/updated_at，每条目录项补 locked/visible。
    let upgraded = with_defaults(
        value,
        &[
            ("tags", Value::Array(Vec::new())),
            ("updated_at", Value::Integer(0)),
        ],
    );
    let upgraded = set(&upgraded, "format", Value::Integer(FORMAT));
    Ok(map_entries(&upgraded, |entry| {
        with_defaults(
            entry,
            &[
                ("locked", Value::Bool(false)),
                ("visible", Value::Bool(true)),
            ],
        )
    }))
}

/// 把块值升到当前格式。格式 1 的正文编码与 2 相同，只需要改格式号。
pub(crate) fn block(value: &Value) -> Result<Value, Error> {
    let found = format::format_of(value)?;
    if found >= FORMAT {
        return Ok(value.clone());
    }
    Ok(set(value, "format", Value::Integer(FORMAT)))
}

/// 缺哪个键就补哪个键；已有的值和未知字段都不动。
fn with_defaults(value: &Value, defaults: &[(&str, Value)]) -> Value {
    let Some(pairs) = value.as_map() else {
        return value.clone();
    };
    let mut out = pairs.to_vec();
    for (key, default) in defaults {
        let present = out
            .iter()
            .any(|(name, _)| matches!(name, Value::Text(name) if name.as_str() == *key));
        if !present {
            out.push((Value::text(*key), default.clone()));
        }
    }
    Value::Map(out)
}

/// 把某个键换成新值；没有这个键就补在末尾。
fn set(value: &Value, key: &str, replacement: Value) -> Value {
    let Some(pairs) = value.as_map() else {
        return value.clone();
    };
    let mut out = Vec::with_capacity(pairs.len().max(1));
    let mut replaced = false;
    for (name, item) in pairs {
        if !replaced && matches!(name, Value::Text(text) if text.as_str() == key) {
            out.push((name.clone(), replacement.clone()));
            replaced = true;
        } else {
            out.push((name.clone(), item.clone()));
        }
    }
    if !replaced {
        out.push((Value::text(key), replacement));
    }
    Value::Map(out)
}

/// 对目录数组里的每一条目录项做一次改动；不是画板映射就原样返回。
fn map_entries(value: &Value, change: impl Fn(&Value) -> Value) -> Value {
    let Some(pairs) = value.as_map() else {
        return value.clone();
    };
    let mut out = Vec::with_capacity(pairs.len());
    for (name, item) in pairs {
        let updated = match (name, item) {
            (Value::Text(name), Value::Array(entries)) if name == "entries" => {
                Value::Array(entries.iter().map(&change).collect())
            }
            _ => item.clone(),
        };
        out.push((name.clone(), updated));
    }
    Value::Map(out)
}

#[cfg(test)]
mod tests {
    use super::{block, board};
    use crate::cbor::Value;
    use crate::format::{self, FORMAT};

    fn format_v1_board() -> Value {
        Value::map(vec![
            ("format", Value::Integer(1)),
            ("kind", Value::text("board")),
            ("title", Value::text("旧画板")),
            (
                "entries",
                Value::Array(vec![Value::map(vec![
                    ("id", Value::Integer(1)),
                    ("block_type", Value::Integer(1)),
                    ("x", Value::Float(0.0)),
                    ("y", Value::Float(0.0)),
                    ("w", Value::Float(10.0)),
                    ("h", Value::Float(10.0)),
                ])]),
            ),
            ("future_board", Value::text("留着")),
        ])
    }

    #[test]
    fn format_one_board_gets_the_new_fields() {
        let upgraded = board(&format_v1_board()).expect("能迁移");
        assert_eq!(
            upgraded.get("format").and_then(Value::as_integer),
            Some(FORMAT)
        );
        assert_eq!(
            upgraded.get("updated_at").and_then(Value::as_integer),
            Some(0)
        );
        assert_eq!(
            upgraded
                .get("tags")
                .and_then(Value::as_array)
                .map(|tags| tags.len()),
            Some(0)
        );
        assert_eq!(
            upgraded.get("future_board").and_then(Value::as_text),
            Some("留着")
        );
        let entries = upgraded
            .get("entries")
            .and_then(Value::as_array)
            .expect("有目录");
        let entry = &entries[0];
        assert_eq!(entry.get("locked").and_then(Value::as_bool), Some(false));
        assert_eq!(entry.get("visible").and_then(Value::as_bool), Some(true));
    }

    #[test]
    fn format_two_board_is_returned_unchanged() {
        let current = Value::map(vec![
            ("format", Value::Integer(FORMAT)),
            ("title", Value::text("新画板")),
            ("updated_at", Value::Integer(1234)),
            ("entries", Value::Array(Vec::new())),
        ]);
        assert_eq!(board(&current).expect("是当前格式"), current);
    }

    #[test]
    fn newer_format_is_refused() {
        let future = Value::map(vec![("format", Value::Integer(FORMAT + 1))]);
        assert!(board(&future).is_err());
        assert!(block(&future).is_err());
    }

    #[test]
    fn format_one_block_only_changes_the_format_number() {
        let old = Value::map(vec![
            ("format", Value::Integer(1)),
            ("block_type", Value::Integer(1)),
            ("id", Value::Integer(1)),
            ("text", Value::text("旧正文")),
            ("future", Value::Bool(true)),
        ]);
        let upgraded = block(&old).expect("能迁移");
        assert_eq!(
            upgraded.get("format").and_then(Value::as_integer),
            Some(FORMAT)
        );
        assert_eq!(
            upgraded.get("text").and_then(Value::as_text),
            Some("旧正文")
        );
        assert_eq!(upgraded.get("future").and_then(Value::as_bool), Some(true));
        assert_eq!(format::format_of(&upgraded).expect("格式号有效"), FORMAT);
    }
}
