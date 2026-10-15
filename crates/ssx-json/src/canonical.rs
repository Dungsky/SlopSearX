//! Sorted Python-compatible JSON byte profiles for cross-runtime serialization.
use crate::{Error, Value};
fn codec_error(message: &str) -> Error {
    <Error as serde::de::Error>::custom(message)
}
#[derive(Debug, Clone, Copy)]
pub enum JsonProfile {
    AsciiCompact,
    Utf8Compact,
    AsciiSpaced,
}
impl JsonProfile {
    fn ascii(self) -> bool {
        !matches!(self, Self::Utf8Compact)
    }
    fn spaced(self) -> bool {
        matches!(self, Self::AsciiSpaced)
    }
}
fn string(value: &str, ascii: bool, out: &mut String) {
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 32 || (ascii && ((c as u32) >= 127)) => {
                for unit in c.encode_utf16(&mut [0; 2]).iter() {
                    out.push_str(&format!("\\u{unit:04x}"));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}
/// CPython JSON uses shortest-roundtrip digits with its own fixed/scientific cutoffs.
pub fn python_float(value: f64) -> Result<String, Error> {
    if !value.is_finite() {
        return Err(codec_error("nonfinite durable JSON number"));
    }
    if value == 0.0 {
        return Ok(if value.is_sign_negative() {
            "-0.0"
        } else {
            "0.0"
        }
        .into());
    }
    let mut buffer = ryu::Buffer::new();
    let raw = buffer.format_finite(value.abs());
    let (mantissa, exponent) = raw
        .split_once('e')
        .map_or((raw, 0), |(m, e)| (m, e.parse::<i32>().unwrap()));
    let dot = mantissa.find('.').unwrap_or(mantissa.len()) as i32;
    let all = mantissa.replace('.', "");
    let leading = all.chars().take_while(|c| *c == '0').count();
    let mut digits = all[leading..].trim_end_matches('0').to_owned();
    let power = exponent + dot - leading as i32 - 1;
    if digits.is_empty() {
        digits.push('0');
    }
    let sign = if value.is_sign_negative() { "-" } else { "" };
    let text = if !(-4..16).contains(&power) {
        let mantissa = if digits.len() == 1 {
            digits.clone()
        } else {
            format!("{}.{}", &digits[..1], &digits[1..])
        };
        format!(
            "{mantissa}e{}{:02}",
            if power >= 0 { "+" } else { "-" },
            power.abs()
        )
    } else {
        let decimal = power + 1;
        if decimal <= 0 {
            format!("0.{}{}", "0".repeat((-decimal) as usize), digits)
        } else if decimal as usize >= digits.len() {
            format!(
                "{}{}.0",
                digits,
                "0".repeat(decimal as usize - digits.len())
            )
        } else {
            format!(
                "{}.{}",
                &digits[..decimal as usize],
                &digits[decimal as usize..]
            )
        }
    };
    Ok(format!("{sign}{text}"))
}
pub fn canonical_json(value: &Value, profile: JsonProfile) -> Result<Vec<u8>, Error> {
    canonical_json_text(
        &crate::to_string(value).map_err(|_| codec_error("invalid JSON"))?,
        profile,
    )
}
/// Parse lossless JSON tokens without serde's private arbitrary-precision map tag.
/// Large integers stay decimal integers and ordinary user dictionaries stay objects.
pub fn canonical_json_text(raw: &str, profile: JsonProfile) -> Result<Vec<u8>, Error> {
    if raw.len() > crate::MAX_JSON_BYTES {
        return Err(codec_error("JSON byte limit exceeded"));
    }
    use serde_json::value::RawValue;
    fn write(raw: &str, p: JsonProfile, out: &mut String, depth: usize) -> Result<(), Error> {
        if depth > 100 {
            return Err(codec_error("durable JSON exceeds depth limit"));
        }
        let raw = raw.trim();
        match raw.as_bytes().first() {
            Some(b'{') => {
                let object: std::collections::BTreeMap<String, Box<RawValue>> =
                    serde_json::from_str(raw).map_err(|_| codec_error("invalid JSON object"))?;
                out.push('{');
                for (index, (key, value)) in object.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                        if p.spaced() {
                            out.push(' ');
                        }
                    }
                    string(key, p.ascii(), out);
                    out.push(':');
                    if p.spaced() {
                        out.push(' ');
                    }
                    write(value.get(), p, out, depth + 1)?;
                }
                out.push('}');
            }
            Some(b'[') => {
                let array: Vec<Box<RawValue>> =
                    serde_json::from_str(raw).map_err(|_| codec_error("invalid JSON array"))?;
                out.push('[');
                for (index, value) in array.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                        if p.spaced() {
                            out.push(' ');
                        }
                    }
                    write(value.get(), p, out, depth + 1)?;
                }
                out.push(']');
            }
            Some(b'"') => {
                let value: String =
                    serde_json::from_str(raw).map_err(|_| codec_error("invalid JSON string"))?;
                string(&value, p.ascii(), out);
            }
            _ => {
                let _: Box<RawValue> =
                    serde_json::from_str(raw).map_err(|_| codec_error("invalid JSON token"))?;
                if ["true", "false", "null"].contains(&raw) {
                    out.push_str(raw);
                } else if raw.contains(['.', 'e', 'E']) {
                    out.push_str(&python_float(
                        raw.parse().map_err(|_| codec_error("invalid float"))?,
                    )?);
                } else {
                    out.push_str(if raw == "-0" { "0" } else { raw });
                }
            }
        }
        Ok(())
    }
    let mut out = String::new();
    write(raw, profile, &mut out, 0)?;
    Ok(out.into_bytes())
}
