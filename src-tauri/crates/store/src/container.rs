//! 容器：把一张画板用得上的文件打进一个文件，整张带走或备份。
//!
//! 格式自己定，不引依赖：魔数、版本号，然后一串「相对地址 + 内容长度 + 内容」。
//! 相对地址在解码时就要校验，容器也可能来自别处，不能直接信。

use crate::error::Error;
use crate::paths;

const MAGIC: &[u8] = b"NOTECEVIA-CONTAINER\n";
const VERSION: u8 = 2;

/// 容器里的一个文件。
pub(crate) struct ContainerEntry {
    pub path: String,
    pub bytes: Vec<u8>,
}

pub(crate) fn encode(entries: &[ContainerEntry]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    for entry in entries {
        let path = entry.path.as_bytes();
        out.extend_from_slice(&(path.len() as u32).to_le_bytes());
        out.extend_from_slice(path);
        out.extend_from_slice(&(entry.bytes.len() as u64).to_le_bytes());
        out.extend_from_slice(&entry.bytes);
    }
    out
}

pub(crate) fn decode(bytes: &[u8]) -> Result<Vec<ContainerEntry>, Error> {
    let rest = bytes
        .strip_prefix(MAGIC)
        .ok_or(Error::BadContainer("文件开头不是 Notecevia 容器"))?;
    let (&version, mut rest) = rest
        .split_first()
        .ok_or(Error::BadContainer("容器在版本号之前就结束了"))?;
    if version != VERSION {
        return Err(Error::BadContainer("容器版本号不认识"));
    }

    let mut entries = Vec::new();
    while !rest.is_empty() {
        let (length, tail) = read_u32(rest)?;
        let length = usize::try_from(length).map_err(|_| Error::BadContainer("地址太长"))?;
        let path_bytes = tail
            .get(..length)
            .ok_or(Error::BadContainer("地址不完整"))?;
        rest = &tail[length..];
        let path = core::str::from_utf8(path_bytes)
            .map_err(|_| Error::BadContainer("地址不是合法的 UTF-8"))?;
        if !paths::is_inside(path) {
            return Err(Error::BadContainer("容器里有不在工作区内的地址"));
        }
        let (length, tail) = read_u64(rest)?;
        let length = usize::try_from(length).map_err(|_| Error::BadContainer("内容太长"))?;
        let bytes = tail
            .get(..length)
            .ok_or(Error::BadContainer("内容不完整"))?
            .to_vec();
        rest = &tail[length..];
        entries.push(ContainerEntry {
            path: path.to_owned(),
            bytes,
        });
    }
    Ok(entries)
}

fn read_u32(bytes: &[u8]) -> Result<(u32, &[u8]), Error> {
    let head = bytes
        .get(..4)
        .ok_or(Error::BadContainer("长度字段不完整"))?;
    let value = u32::from_le_bytes([head[0], head[1], head[2], head[3]]);
    Ok((value, &bytes[4..]))
}

fn read_u64(bytes: &[u8]) -> Result<(u64, &[u8]), Error> {
    let head = bytes
        .get(..8)
        .ok_or(Error::BadContainer("长度字段不完整"))?;
    let value = u64::from_le_bytes([
        head[0], head[1], head[2], head[3], head[4], head[5], head[6], head[7],
    ]);
    Ok((value, &bytes[8..]))
}

#[cfg(test)]
mod tests {
    use super::{ContainerEntry, decode, encode};

    fn sample() -> Vec<ContainerEntry> {
        vec![
            ContainerEntry {
                path: "boards/1.board".to_owned(),
                bytes: vec![1, 2, 3],
            },
            ContainerEntry {
                path: "media/abc".to_owned(),
                bytes: b"media bytes".to_vec(),
            },
        ]
    }

    #[test]
    fn entries_roundtrip() {
        let entries = sample();
        let decoded = decode(&encode(&entries)).expect("容器能读回来");
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].path, "boards/1.board");
        assert_eq!(decoded[0].bytes, vec![1, 2, 3]);
        assert_eq!(decoded[1].path, "media/abc");
        assert_eq!(decoded[1].bytes, b"media bytes");
    }

    #[test]
    fn foreign_files_are_refused() {
        assert!(decode(b"not a container").is_err());
        assert!(decode(b"").is_err());
    }

    #[test]
    fn escapes_inside_the_container_are_refused() {
        let entries = vec![ContainerEntry {
            path: "../../etc/passwd".to_owned(),
            bytes: Vec::new(),
        }];
        assert!(decode(&encode(&entries)).is_err());
    }

    #[test]
    fn truncated_content_is_an_error() {
        let mut bytes = encode(&sample());
        bytes.truncate(bytes.len() - 1);
        assert!(decode(&bytes).is_err());
    }
}
