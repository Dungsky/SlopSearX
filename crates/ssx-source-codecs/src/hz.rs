// SPDX-License-Identifier: MIT AND PSF-2.0 AND Unicode-3.0
// CPython3.14.7 Modules/cjkcodecs/_codecs_cn.c DECODER(hz),
// final-input/errors=replace. Change: owned codepoints replace Unicode writer;
// invalid and incomplete source consume rules remain explicit.
use crate::Text;
pub(crate) fn decode(bytes: &[u8]) -> Text {
    decode_mode(bytes, false).expect("replacement HZ returns text")
}
pub(crate) fn decode_strict(bytes: &[u8]) -> Option<Text> {
    decode_mode(bytes, true)
}
fn decode_mode(bytes: &[u8], strict: bool) -> Option<Text> {
    let mut chinese = false;
    let mut cursor = 0;
    let mut out = Vec::with_capacity(bytes.len());
    while cursor < bytes.len() {
        let first = bytes[cursor];
        if first == b'~' {
            let Some(&second) = bytes.get(cursor + 1) else {
                if strict {
                    return None;
                }
                out.push(0xfffd);
                break;
            };
            match (second, chinese) {
                (b'~', false) => out.push(126),
                (b'{', false) => chinese = true,
                (b'\n', false) => {}
                (b'}', true) => chinese = false,
                _ => {
                    if strict {
                        return None;
                    }
                    out.push(0xfffd);
                    cursor += 1;
                    continue;
                }
            }
            cursor += 2;
            continue;
        }
        if first >= 0x80 {
            if strict {
                return None;
            }
            out.push(0xfffd);
            cursor += 1;
            continue;
        }
        if !chinese {
            out.push(u32::from(first));
            cursor += 1;
            continue;
        }
        let Some(&second) = bytes.get(cursor + 1) else {
            if strict {
                return None;
            }
            out.push(0xfffd);
            break;
        };
        if (0x21..=0x7e).contains(&first)
            && (0x21..=0x7e).contains(&second)
            && let Ok(text) =
                crate::doublebyte::decode_strict(&[first | 0x80, second | 0x80], "gb2312").unwrap()
            && text.codepoints().len() == 1
        {
            out.push(text.codepoints()[0]);
            cursor += 2;
            continue;
        }
        if strict {
            return None;
        }
        out.push(0xfffd);
        cursor += 1;
    }
    Some(Text::from_codepoints(out).unwrap())
}
