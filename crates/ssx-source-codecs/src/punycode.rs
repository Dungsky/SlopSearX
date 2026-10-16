// SPDX-License-Identifier: MIT AND PSF-2.0
// Native errors=replace port of CPython 3.14.7 Lib/encodings/punycode.py.
// Change: Python arbitrary integers use private unsigned base-2^32 limbs;
// Python strings use owned codepoints, preserving non-scalar surrogates.
use crate::Text;
#[derive(Clone, Default)]
struct Number(Vec<u32>);
impl Number {
    fn small(n: u32) -> Self {
        Self(vec![n])
    }
    fn trim(&mut self) {
        while self.0.last() == Some(&0) {
            self.0.pop();
        }
    }
    fn add(&mut self, other: &Self) {
        let mut carry = 0u64;
        self.0.resize(self.0.len().max(other.0.len()), 0);
        for i in 0..self.0.len() {
            let n = u64::from(self.0[i]) + u64::from(other.0.get(i).copied().unwrap_or(0)) + carry;
            self.0[i] = n as u32;
            carry = n >> 32;
        }
        if carry != 0 {
            self.0.push(carry as u32);
        }
    }
    fn mul(&mut self, n: u32) {
        let mut carry = 0u64;
        for x in &mut self.0 {
            let v = u64::from(*x) * u64::from(n) + carry;
            *x = v as u32;
            carry = v >> 32;
        }
        if carry != 0 {
            self.0.push(carry as u32);
        }
        self.trim();
    }
    fn div(&mut self, n: u32) -> u32 {
        let mut remainder = 0u64;
        for x in self.0.iter_mut().rev() {
            let v = (remainder << 32) | u64::from(*x);
            *x = (v / u64::from(n)) as u32;
            remainder = v % u64::from(n);
        }
        self.trim();
        remainder as u32
    }
    fn bounded(&self, max: u32) -> Option<u32> {
        if self.0.len() > 1 {
            None
        } else {
            let n = self.0.first().copied().unwrap_or(0);
            (n <= max).then_some(n)
        }
    }
    fn bounded_u64(&self) -> Option<u64> {
        if self.0.len() > 2 {
            None
        } else {
            Some(
                u64::from(self.0.first().copied().unwrap_or(0))
                    | (u64::from(self.0.get(1).copied().unwrap_or(0)) << 32),
            )
        }
    }
}
fn adapt(mut delta: Number, first: bool, count: u32) -> usize {
    delta.div(if first { 700 } else { 2 });
    let mut extra = delta.clone();
    extra.div(count);
    delta.add(&extra);
    let mut divisions = 0;
    while delta.bounded(455).is_none() {
        delta.div(35);
        divisions += 36;
    }
    let d = delta.bounded(455).unwrap();
    divisions + (36 * d / (d + 38)) as usize
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StrictError {
    Unicode { end: u64 },
    Overflow,
}
fn failure(end: u64, offset: u64) -> StrictError {
    match end.checked_add(offset).filter(|n| *n <= isize::MAX as u64) {
        Some(end) => StrictError::Unicode { end },
        None => StrictError::Overflow,
    }
}
pub(crate) fn offset_error(error: StrictError, offset: u64) -> StrictError {
    match error {
        StrictError::Unicode { end } => failure(end, offset),
        StrictError::Overflow => StrictError::Overflow,
    }
}
pub(crate) fn decode(bytes: &[u8]) -> Text {
    decode_mode(bytes, false).expect("replacement punycode always returns text")
}
pub(crate) fn decode_strict(bytes: &[u8]) -> Result<Text, StrictError> {
    decode_mode(bytes, true)
}
fn decode_mode(bytes: &[u8], strict: bool) -> Result<Text, StrictError> {
    let split = bytes.iter().rposition(|b| *b == b'-');
    let offset = split.map_or(0, |n| n + 1) as u64;
    let (mut base, extended) = match split {
        Some(i) => {
            if strict && let Some(index) = bytes[..i].iter().position(|b| !b.is_ascii()) {
                return Err(failure(index as u64 + 1, 0));
            }
            let mut base = Vec::with_capacity(bytes.len());
            base.extend(
                bytes[..i]
                    .iter()
                    .map(|b| if b.is_ascii() { u32::from(*b) } else { 0xfffd }),
            );
            (base, &bytes[i + 1..])
        }
        None => (Vec::with_capacity(bytes.len()), bytes),
    };
    let mut character = 128;
    let mut position = None;
    let mut bias = 72;
    let mut cursor = 0;
    while cursor < extended.len() {
        let start = cursor;
        let mut delta = Number::default();
        let mut weight = Number::small(1);
        let mut j = 0usize;
        loop {
            let Some(byte) = extended.get(cursor) else {
                return if strict {
                    Err(failure(cursor as u64 + 1, offset))
                } else {
                    Ok(Text::from_codepoints(base).unwrap())
                };
            };
            cursor += 1;
            let digit = match byte.to_ascii_uppercase() {
                b'A'..=b'Z' => u32::from(byte.to_ascii_uppercase() - b'A'),
                b'0'..=b'9' => u32::from(*byte - 22),
                _ => {
                    return if strict {
                        Err(failure(cursor as u64, offset))
                    } else {
                        Ok(Text::from_codepoints(base).unwrap())
                    };
                }
            };
            let threshold =
                (36usize.saturating_mul(j + 1).saturating_sub(bias)).clamp(1, 26) as u32;
            let mut term = weight.clone();
            term.mul(digit);
            delta.add(&term);
            if digit < threshold {
                break;
            }
            weight.mul(36 - threshold);
            j += 1;
        }
        let mut next = delta.clone();
        if let Some(previous) = position {
            next.add(&Number::small(previous + 1));
        }
        let error_end = next.bounded_u64();
        let count =
            u32::try_from(base.len() + 1).expect("admitted parameter input bounds insertions");
        let at = next.div(count);
        character = match next.bounded(0x10ffff - character) {
            Some(increment) => character + increment,
            None if strict => {
                return Err(error_end.map_or(StrictError::Overflow, |end| failure(end, offset)));
            }
            None => 63,
        };
        position = Some(at);
        base.insert(at as usize, character);
        bias = adapt(delta, start == 0, count);
    }
    Ok(Text::from_codepoints(base).unwrap())
}
