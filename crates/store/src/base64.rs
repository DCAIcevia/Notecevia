//! base64：HTML 导出的图片用 `data:` URI 内嵌，需要把字节写成 ASCII。
//!
//! 只用标准字母表与 `=` 补位（RFC 4648 §4）；解码暂时用不到，不写。

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// 把字节编码成 base64 文本。空输入给空串。
pub fn encode(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        text.push(char::from(ALPHABET[usize::from(first >> 2)]));
        text.push(char::from(
            ALPHABET[usize::from(((first & 0x03) << 4) | (second >> 4))],
        ));
        if chunk.len() > 1 {
            text.push(char::from(
                ALPHABET[usize::from(((second & 0x0f) << 2) | (third >> 6))],
            ));
        } else {
            text.push('=');
        }
        if chunk.len() > 2 {
            text.push(char::from(ALPHABET[usize::from(third & 0x3f)]));
        } else {
            text.push('=');
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::encode;

    #[test]
    fn rfc_4648_vectors_are_matched() {
        assert_eq!(encode(b""), "");
        assert_eq!(encode(b"f"), "Zg==");
        assert_eq!(encode(b"fo"), "Zm8=");
        assert_eq!(encode(b"foo"), "Zm9v");
        assert_eq!(encode(b"foob"), "Zm9vYg==");
        assert_eq!(encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn binary_bytes_are_encoded_byte_wise() {
        assert_eq!(encode(&[0x00]), "AA==");
        assert_eq!(encode(&[0xff]), "/w==");
        assert_eq!(encode(&[0x00, 0xff, 0x10]), "AP8Q");
    }
}
