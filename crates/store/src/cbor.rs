//! CBOR 子集：只实现格式需要的那几种类型。
//!
//! 完整 CBOR 有八种主类型、定长与不定长两种写法。这里只需要整数、文本、数组、
//! 映射和浮点，而且一律写成定长形式。解码遇到别的类型直接报错——
//! 读不懂就停下来说清楚，不猜、不丢。
//!
//! 产物是标准 CBOR（RFC 8949），别的工具能读。

use thiserror::Error;

/// 一个编码好的值。
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Integer(i64),
    Float(f64),
    Bool(bool),
    Text(String),
    Array(Vec<Value>),
    /// 保持写入顺序，不排序：映射都很小，字节稳定比查找快重要。
    Map(Vec<(Value, Value)>),
}

impl Value {
    /// 用文本键建一个映射。
    pub fn map(pairs: Vec<(&str, Value)>) -> Self {
        let pairs = pairs
            .into_iter()
            .map(|(key, value)| (Self::Text(key.to_owned()), value))
            .collect();
        Self::Map(pairs)
    }

    /// 用文本键建映射，末尾接上原样保留的未知字段。
    pub fn map_with_extras(pairs: Vec<(&str, Value)>, extras: Vec<(Value, Value)>) -> Self {
        let mut map: Vec<(Value, Value)> = pairs
            .into_iter()
            .map(|(key, value)| (Self::Text(key.to_owned()), value))
            .collect();
        map.extend(extras);
        Self::Map(map)
    }

    pub fn text(value: impl Into<String>) -> Self {
        Self::Text(value.into())
    }

    /// 映射的键值对；不是映射给 None。
    pub fn as_map(&self) -> Option<&[(Value, Value)]> {
        match self {
            Self::Map(pairs) => Some(pairs),
            _ => None,
        }
    }

    /// 取映射里的一个键，非映射或没有该键都返回 None。
    pub fn get(&self, key: &str) -> Option<&Value> {
        let Self::Map(pairs) = self else {
            return None;
        };
        pairs.iter().find_map(|(name, value)| match name {
            Self::Text(name) if name == key => Some(value),
            _ => None,
        })
    }

    pub fn as_integer(&self) -> Option<i64> {
        match self {
            Self::Integer(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            Self::Float(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Self::Array(values) => Some(values),
            _ => None,
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CborError {
    #[error("数据在读完之前就结束了")]
    Truncated,
    #[error("不支持的主类型 {0}")]
    UnsupportedMajor(u8),
    #[error("不支持的长度写法：{0}")]
    UnsupportedLength(u8),
    #[error("长度超出这个平台能分配的范围")]
    TooLarge,
    #[error("整数超出 i64 的范围")]
    IntegerOverflow,
    #[error("文本不是合法的 UTF-8")]
    InvalidUtf8,
    #[error("值后面还有多余的数据")]
    TrailingData,
}

pub fn encode(value: &Value) -> Vec<u8> {
    let mut out = Vec::new();
    write(value, &mut out);
    out
}

fn write(value: &Value, out: &mut Vec<u8>) {
    match value {
        Value::Integer(number) if *number >= 0 => head(0, *number as u64, out),
        Value::Integer(number) => head(1, (-1 - *number) as u64, out),
        // 布尔用主类型 7 的两个短形式位，和 CBOR 的规定一致。
        Value::Bool(false) => out.push(0xf4),
        Value::Bool(true) => out.push(0xf5),
        Value::Float(number) => {
            out.push(0xfb);
            out.extend_from_slice(&number.to_bits().to_be_bytes());
        }
        Value::Text(text) => {
            head(3, text.len() as u64, out);
            out.extend_from_slice(text.as_bytes());
        }
        Value::Array(items) => {
            head(4, items.len() as u64, out);
            for item in items {
                write(item, out);
            }
        }
        Value::Map(pairs) => {
            head(5, pairs.len() as u64, out);
            for (key, item) in pairs {
                write(key, out);
                write(item, out);
            }
        }
    }
}

/// 写主类型和长度。长度一律用最短的定长形式。
fn head(major: u8, length: u64, out: &mut Vec<u8>) {
    let major = major << 5;
    if length < 24 {
        out.push(major | length as u8);
    } else if let Ok(value) = u8::try_from(length) {
        out.push(major | 24);
        out.push(value);
    } else if let Ok(value) = u16::try_from(length) {
        out.push(major | 25);
        out.extend_from_slice(&value.to_be_bytes());
    } else if let Ok(value) = u32::try_from(length) {
        out.push(major | 26);
        out.extend_from_slice(&value.to_be_bytes());
    } else {
        out.push(major | 27);
        out.extend_from_slice(&length.to_be_bytes());
    }
}

pub fn decode(bytes: &[u8]) -> Result<Value, CborError> {
    let mut reader = Reader { bytes, pos: 0 };
    let value = reader.value()?;
    if reader.pos != bytes.len() {
        return Err(CborError::TrailingData);
    }
    Ok(value)
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn take(&mut self, count: usize) -> Result<&[u8], CborError> {
        let end = self.pos.checked_add(count).ok_or(CborError::TooLarge)?;
        let slice = self.bytes.get(self.pos..end).ok_or(CborError::Truncated)?;
        self.pos = end;
        Ok(slice)
    }

    /// 读固定长度的字节块。长度由 CBOR 的规定给出，读不满就是数据坏了。
    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], CborError> {
        let bytes = self.take(N)?;
        bytes.try_into().map_err(|_| CborError::Truncated)
    }

    /// 读主类型和长度。浮点的长度位被当作 8 字节读进来，由调用方按主类型解释。
    fn head(&mut self) -> Result<(u8, u64), CborError> {
        let first = self.take(1)?[0];
        let major = first >> 5;
        let length = match first & 0x1f {
            0..=23 => u64::from(first & 0x1f),
            24 => u64::from(self.fixed::<1>()?[0]),
            25 => u64::from(u16::from_be_bytes(self.fixed()?)),
            26 => u64::from(u32::from_be_bytes(self.fixed()?)),
            27 => u64::from_be_bytes(self.fixed()?),
            extra => return Err(CborError::UnsupportedLength(extra)),
        };
        Ok((major, length))
    }

    fn count(&self, length: u64) -> Result<usize, CborError> {
        usize::try_from(length).map_err(|_| CborError::TooLarge)
    }

    fn value(&mut self) -> Result<Value, CborError> {
        let (major, length) = self.head()?;
        match major {
            0 => Ok(Value::Integer(
                i64::try_from(length).map_err(|_| CborError::IntegerOverflow)?,
            )),
            1 => Ok(Value::Integer(
                i64::try_from(length)
                    .ok()
                    .and_then(|value| value.checked_neg())
                    .and_then(|value| value.checked_sub(1))
                    .ok_or(CborError::IntegerOverflow)?,
            )),
            3 => {
                let count = self.count(length)?;
                let bytes = self.take(count)?;
                let text = core::str::from_utf8(bytes).map_err(|_| CborError::InvalidUtf8)?;
                Ok(Value::Text(text.to_owned()))
            }
            4 => {
                let count = self.count(length)?;
                let mut items = Vec::new();
                for _ in 0..count {
                    items.push(self.value()?);
                }
                Ok(Value::Array(items))
            }
            5 => {
                let count = self.count(length)?;
                let mut pairs = Vec::new();
                for _ in 0..count {
                    let key = self.value()?;
                    let item = self.value()?;
                    pairs.push((key, item));
                }
                Ok(Value::Map(pairs))
            }
            // 0xf4 / 0xf5 是假与真，其余长度位按双精度浮点解释。
            7 => match length {
                20 => Ok(Value::Bool(false)),
                21 => Ok(Value::Bool(true)),
                _ => Ok(Value::Float(f64::from_bits(length))),
            },
            other => Err(CborError::UnsupportedMajor(other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integers_use_the_shortest_form() {
        assert_eq!(encode(&Value::Integer(1)), vec![0x01]);
        assert_eq!(encode(&Value::Integer(23)), vec![0x17]);
        assert_eq!(encode(&Value::Integer(24)), vec![0x18, 0x18]);
        assert_eq!(encode(&Value::Integer(300)), vec![0x19, 0x01, 0x2c]);
        assert_eq!(encode(&Value::Integer(-1)), vec![0x20]);
        assert_eq!(encode(&Value::Integer(-2)), vec![0x21]);
        assert_eq!(encode(&Value::Text("a".to_owned())), vec![0x61, b'a']);
    }

    #[test]
    fn bools_use_the_standard_short_form() {
        assert_eq!(encode(&Value::Bool(false)), vec![0xf4]);
        assert_eq!(encode(&Value::Bool(true)), vec![0xf5]);
        assert_eq!(decode(&[0xf4]), Ok(Value::Bool(false)));
        assert_eq!(decode(&[0xf5]), Ok(Value::Bool(true)));
    }

    #[test]
    fn map_keeps_key_order_and_roundtrips() {
        let value = Value::map(vec![
            ("format", Value::Integer(1)),
            ("title", Value::text("画板")),
            ("x", Value::Float(12.5)),
            ("z", Value::Integer(-3)),
            ("entries", Value::Array(vec![Value::Integer(0)])),
        ]);
        let bytes = encode(&value);
        assert_eq!(decode(&bytes), Ok(value));
    }

    #[test]
    fn extras_are_appended_after_the_known_keys() {
        let value = Value::map_with_extras(
            vec![
                ("format", Value::Integer(2)),
                ("title", Value::text("画板")),
            ],
            vec![
                (Value::text("future"), Value::Bool(true)),
                (Value::text("future_number"), Value::Integer(7)),
            ],
        );
        let bytes = encode(&value);
        assert_eq!(decode(&bytes), Ok(value.clone()));
        let pairs = value.as_map().expect("是映射");
        assert_eq!(
            pairs
                .iter()
                .filter_map(|(key, _)| key.as_text())
                .collect::<Vec<_>>(),
            vec!["format", "title", "future", "future_number"]
        );
    }

    #[test]
    fn truncated_input_is_an_error_not_a_guess() {
        let bytes = encode(&Value::Array(vec![Value::Integer(7)]));
        assert_eq!(decode(&bytes[..bytes.len() - 1]), Err(CborError::Truncated));
    }

    #[test]
    fn trailing_bytes_are_rejected() {
        let mut bytes = encode(&Value::Integer(1));
        bytes.push(0);
        assert_eq!(decode(&bytes), Err(CborError::TrailingData));
    }

    #[test]
    fn unsupported_types_are_rejected() {
        // 0x40 是主类型 2（字节串），这个子集不认。
        assert_eq!(decode(&[0x40]), Err(CborError::UnsupportedMajor(2)));
        // 0x5f 是主类型 2 的不定长写法，一开头就该拒。
        assert_eq!(decode(&[0x5f]), Err(CborError::UnsupportedLength(31)));
    }
}
