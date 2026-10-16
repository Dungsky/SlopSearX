// SPDX-License-Identifier: MIT AND PSF-2.0
// Native final-input/errors=replace port of CPython 3.14.7
// Objects/unicodeobject.c _PyUnicode_DecodeUnicodeEscapeInternal.
// Native Unicode16 name/alias tables retain exact source lookup semantics.
use crate::Text;
pub(crate) fn decode(bytes: &[u8]) -> Option<Text> {
    decode_mode(bytes, false)
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
        if first != b'\\' {
            out.push(u32::from(first));
            continue;
        }
        let Some(&marker) = bytes.get(cursor) else {
            if strict {
                return None;
            }
            out.push(0xfffd);
            break;
        };
        cursor += 1;
        let simple = match marker {
            b'\\' => Some(92),
            b'\'' => Some(39),
            b'"' => Some(34),
            b'b' => Some(8),
            b'f' => Some(12),
            b't' => Some(9),
            b'n' => Some(10),
            b'r' => Some(13),
            b'v' => Some(11),
            b'a' => Some(7),
            _ => None,
        };
        if let Some(point) = simple {
            out.push(point);
            continue;
        }
        if marker == b'\n' {
            continue;
        }
        if (b'0'..=b'7').contains(&marker) {
            let mut point = u32::from(marker - b'0');
            for _ in 0..2 {
                if let Some(&digit) = bytes.get(cursor).filter(|b| (b'0'..=b'7').contains(b)) {
                    point = (point << 3) + u32::from(digit - b'0');
                    cursor += 1;
                } else {
                    break;
                }
            }
            out.push(point);
            continue;
        }
        let digits = match marker {
            b'x' => 2,
            b'u' => 4,
            b'U' => 8,
            b'N' => {
                if bytes.get(cursor) != Some(&b'{') {
                    if strict {
                        return None;
                    }
                    out.push(0xfffd);
                    continue;
                }
                cursor += 1;
                let start = cursor;
                while cursor < bytes.len() && bytes[cursor] != b'}' {
                    cursor += 1;
                }
                if cursor == bytes.len() {
                    if strict {
                        return None;
                    }
                    out.push(0xfffd);
                    break;
                }
                if cursor == start {
                    if strict {
                        return None;
                    }
                    out.push(0xfffd);
                    continue;
                }
                let point = lookup_name(&bytes[start..cursor]);
                cursor += 1;
                if strict && point.is_none() {
                    return None;
                }
                out.push(point.unwrap_or(0xfffd));
                continue;
            }
            _ => {
                out.extend([92, u32::from(marker)]);
                continue;
            }
        };
        let mut value = 0;
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
    Some(Text::from_codepoints(out).unwrap())
}

fn lookup_name(bytes: &[u8]) -> Option<u32> {
    if !bytes.is_ascii() {
        return None;
    }
    let name = std::str::from_utf8(bytes).ok()?;
    for (prefix, id) in [("CJK UNIFIED IDEOGRAPH-", 1), ("TANGUT IDEOGRAPH-", 2)] {
        if name
            .get(..prefix.len())
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(prefix))
        {
            let hex = &name[prefix.len()..];
            if !(4..=6).contains(&hex.len())
                || hex.starts_with('0')
                || !hex.bytes().all(|b| b.is_ascii_hexdigit())
            {
                return None;
            }
            let point = u32::from_str_radix(hex, 16).ok()?;
            return crate::unicode_name_ranges::RANGES
                .iter()
                .any(|&(lo, hi, kind)| kind == id && (lo..=hi).contains(&point))
                .then_some(point);
        }
    }
    crate::unicode_names::NAMES
        .binary_search_by(|(candidate, _)| {
            candidate
                .bytes()
                .cmp(bytes.iter().map(u8::to_ascii_uppercase))
        })
        .ok()
        .map(|i| crate::unicode_names::NAMES[i].1)
}
