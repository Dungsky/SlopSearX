// SPDX-License-Identifier: MIT AND PSF-2.0
// Native port of CPython 3.14.7 Objects/unicodeobject.c
// _PyUnicode_DecodeRawUnicodeEscapeStateful, final input/errors=replace.
// Change: owned u32 codepoints replace the Python Unicode writer/error callback.
use crate::Text;
pub(crate) fn decode(bytes: &[u8]) -> Text {
    decode_mode(bytes, false).expect("replacement rawUnicodeEscape returns text")
}
pub(crate) fn decode_strict(bytes: &[u8]) -> Option<Text> {
    decode_mode(bytes, true)
}
fn decode_mode(bytes: &[u8], strict: bool) -> Option<Text> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut cursor = 0;
    while cursor < bytes.len() {
        let first = bytes[cursor];
        cursor += 1;
        if first != b'\\' || cursor == bytes.len() {
            out.push(u32::from(first));
            continue;
        }
        let marker = bytes[cursor];
        cursor += 1;
        let digits = match marker {
            b'u' => 4,
            b'U' => 8,
            _ => {
                out.extend([u32::from(b'\\'), u32::from(marker)]);
                continue;
            }
        };
        let mut value = 0u32;
        let mut valid = true;
        for _ in 0..digits {
            let Some(digit) = bytes.get(cursor).and_then(|b| char::from(*b).to_digit(16)) else {
                valid = false;
                break;
            };
            value = (value << 4) | digit;
            cursor += 1;
        }
        if strict && (!valid || value > 0x10ffff) {
            return None;
        }
        out.push(if valid && value <= 0x10ffff {
            value
        } else {
            0xfffd
        });
    }
    Some(Text::from_codepoints(out).expect("decoder checks Unicode range"))
}
