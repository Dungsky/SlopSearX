//! Private source IDNA2003 validation for RFC2231 strict decoding.
//! Original algorithm/data notices are retained in provenance/idna/.
use crate::{idna_data as data, punycode};
fn mapping<'a>(c: u32, index: &[(u32, usize, usize)], values: &'a [u32]) -> Option<&'a [u32]> {
    index.binary_search_by_key(&c, |r| r.0).ok().map(|i| {
        let (_, at, len) = index[i];
        &values[at..at + len]
    })
}
fn flags(c: u32) -> u8 {
    let i = data::FLAGS.partition_point(|r| r.0 <= c);
    if i != 0 && c < data::FLAGS[i - 1].1 {
        data::FLAGS[i - 1].2
    } else {
        0
    }
}
fn ccc(c: u32) -> u8 {
    data::CCC
        .binary_search_by_key(&c, |r| r.0)
        .map_or(0, |i| data::CCC[i].1)
}
fn decompose(c: u32, out: &mut Vec<u32>) {
    if (0xac00..0xd7a4).contains(&c) {
        let i = c - 0xac00;
        out.extend([0x1100 + i / 588, 0x1161 + (i % 588) / 28]);
        if !i.is_multiple_of(28) {
            out.push(0x11a7 + i % 28);
        }
    } else if let Some(seq) = mapping(c, data::DECOMP_INDEX, data::DECOMP_DATA) {
        for &p in seq {
            decompose(p, out);
        }
    } else {
        out.push(c);
    }
}
fn compose(a: u32, b: u32) -> Option<u32> {
    if (0x1100..0x1113).contains(&a) && (0x1161..0x1176).contains(&b) {
        return Some(0xac00 + ((a - 0x1100) * 21 + b - 0x1161) * 28);
    }
    if (0xac00..0xd7a4).contains(&a)
        && (a - 0xac00).is_multiple_of(28)
        && (0x11a8..0x11c3).contains(&b)
    {
        return Some(a + b - 0x11a7);
    }
    data::COMPOSE
        .binary_search_by_key(&(a, b), |r| (r.0, r.1))
        .ok()
        .map(|i| data::COMPOSE[i].2)
}
fn nfkc(input: &[u32]) -> Vec<u32> {
    let mut decomposed = Vec::new();
    for &c in input {
        decompose(c, &mut decomposed);
    }
    for i in 1..decomposed.len() {
        let order = ccc(decomposed[i]);
        let mut at = i;
        while order != 0 && at != 0 && ccc(decomposed[at - 1]) > order {
            decomposed.swap(at, at - 1);
            at -= 1;
        }
    }
    let Some(&first) = decomposed.first() else {
        return decomposed;
    };
    // Composition never increases length. Compact into the already-owned
    // decomposition buffer instead of allocating a second full point Vec.
    let mut written = 1;
    let mut starter = 0;
    let mut last = ccc(first);
    for read in 1..decomposed.len() {
        let c = decomposed[read];
        let order = ccc(c);
        if let Some(joined) = compose(decomposed[starter], c).filter(|_| last < order || last == 0)
        {
            decomposed[starter] = joined;
        } else {
            if order == 0 {
                starter = written;
            }
            decomposed[written] = c;
            written += 1;
            last = order;
        }
    }
    decomposed.truncate(written);
    decomposed
}
fn nameprep(input: &[u32]) -> Option<Vec<u32>> {
    let mut mapped = Vec::new();
    for &c in input {
        if flags(c) & 1 == 0 {
            if let Some(seq) = mapping(c, data::B2_INDEX, data::B2_DATA) {
                mapped.extend_from_slice(seq);
            } else {
                mapped.push(c);
            }
        }
    }
    let out = nfkc(&mapped);
    drop(mapped);
    if out.iter().any(|&c| flags(c) & 2 != 0) {
        return None;
    }
    if out.iter().any(|&c| flags(c) & 4 != 0)
        && (out.iter().any(|&c| flags(c) & 8 != 0)
            || flags(*out.first()?) & 4 == 0
            || flags(*out.last()?) & 4 == 0)
    {
        return None;
    }
    Some(out)
}
fn adapt(mut delta: u64, first: bool, count: u64) -> u64 {
    delta /= if first { 700 } else { 2 };
    delta += delta / count;
    let mut k = 0;
    while delta > 455 {
        delta /= 35;
        k += 36;
    }
    k + 36 * delta / (delta + 38)
}
fn encode(points: &[u32]) -> Option<Vec<u8>> {
    let mut out: Vec<u8> = points
        .iter()
        .filter(|&&c| c < 128)
        .map(|&c| c as u8)
        .collect();
    let basic = out.len();
    let mut handled = basic;
    if basic != 0 {
        out.push(b'-');
    }
    let mut n = 128u64;
    let mut delta = 0u64;
    let mut bias = 72u64;
    while handled < points.len() {
        let next = u64::from(*points.iter().filter(|&&c| u64::from(c) >= n).min()?);
        delta = delta.checked_add((next - n).checked_mul(handled as u64 + 1)?)?;
        n = next;
        for &c in points {
            let c = u64::from(c);
            if c < n {
                delta = delta.checked_add(1)?;
            }
            if c == n {
                let mut q = delta;
                let mut k = 36u64;
                loop {
                    let t = k.saturating_sub(bias).clamp(1, 26);
                    let digit = if q < t { q } else { t + (q - t) % (36 - t) };
                    out.push(if digit < 26 {
                        b'a' + digit as u8
                    } else {
                        b'0' + (digit - 26) as u8
                    });
                    if q < t {
                        break;
                    }
                    q = (q - t) / (36 - t);
                    k = k.checked_add(36)?;
                }
                bias = adapt(delta, handled == basic, handled as u64 + 1);
                delta = 0;
                handled += 1;
            }
        }
        delta = delta.checked_add(1)?;
        n = n.checked_add(1)?;
    }
    Some(out)
}
fn to_ascii(input: &[u32]) -> Option<Vec<u8>> {
    if input.iter().all(|&c| c < 128) {
        return (1..64)
            .contains(&input.len())
            .then(|| input.iter().map(|&c| c as u8).collect());
    }
    let prepared = nameprep(input)?;
    if prepared.iter().all(|&c| c < 128) {
        return (1..64)
            .contains(&prepared.len())
            .then(|| prepared.iter().map(|&c| c as u8).collect());
    }
    if prepared.get(..4).is_some_and(|s| {
        s.iter()
            .zip(b"xn--")
            .all(|(&a, &b)| a < 128 && (a as u8).eq_ignore_ascii_case(&b))
    }) {
        return None;
    }
    let mut encoded = b"xn--".to_vec();
    encoded.extend(encode(&prepared)?);
    (encoded.len() < 64).then_some(encoded)
}
/// Successful ASCII/non-ASCII classification after validating every label.
/// Unicode failures remain distinguishable from propagated position overflow.
pub(crate) fn ascii_outcome(bytes: &[u8]) -> Result<bool, punycode::StrictError> {
    use punycode::StrictError;
    if bytes.is_empty() {
        return Ok(true);
    }
    if !bytes.windows(4).any(|s| s.eq_ignore_ascii_case(b"xn--")) {
        return if bytes.is_ascii() {
            Ok(true)
        } else {
            Err(StrictError::Unicode { end: 0 })
        };
    }
    let input = bytes.strip_suffix(b".").unwrap_or(bytes);
    let mut ascii = true;
    let mut offset = 0u64;
    for label in input.split(|&c| c == b'.') {
        if label.len() > 1024 {
            return Err(StrictError::Unicode { end: 0 });
        }
        if !label
            .get(..4)
            .is_some_and(|s| s.eq_ignore_ascii_case(b"xn--"))
        {
            if !label.is_ascii() {
                return Err(StrictError::Unicode { end: 0 });
            }
        } else {
            let decoded = punycode::decode_strict(&label[4..])
                .map_err(|e| punycode::offset_error(e, 4 + offset))?;
            let encoded = to_ascii(decoded.codepoints()).ok_or(StrictError::Unicode { end: 0 })?;
            if !encoded.eq_ignore_ascii_case(label) {
                return Err(StrictError::Unicode { end: 0 });
            }
            ascii &= decoded.codepoints().iter().all(|&c| c < 128);
        }
        offset = offset
            .checked_add(label.len() as u64 + 1)
            .ok_or(StrictError::Overflow)?;
    }
    Ok(ascii)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn without_overflow(bytes: &[u8]) -> Option<bool> {
        match ascii_outcome(bytes) {
            Ok(value) => Some(value),
            Err(punycode::StrictError::Unicode { .. }) => None,
            Err(punycode::StrictError::Overflow) => {
                panic!("unexpected overflow in original source subset")
            }
        }
    }

    #[test]
    fn strict_idna_all_source_parameters_preserve_unicode_failure_and_ascii_outcome() {
        let source: serde_json::Value =
            serde_json::from_str(include_str!("../tests/idna-direct-source.json")).unwrap();
        let rows = source["cases"].as_array().unwrap();
        assert_eq!(rows.len(), 488);
        let mut ids = std::collections::BTreeSet::new();
        for row in rows {
            let original = &row["whole_httpx_source"];
            assert!(ids.insert(original["id"].as_str().unwrap()));
            let hex = original["parameter_bytes_hex"].as_str().unwrap();
            let bytes: Vec<u8> = hex
                .as_bytes()
                .as_chunks::<2>()
                .0
                .iter()
                .map(|s| u8::from_str_radix(std::str::from_utf8(s).unwrap(), 16).unwrap())
                .collect();
            let expected = row["decoded_codepoints"]
                .as_array()
                .map(|points| points.iter().all(|p| p.as_u64().unwrap() < 128));
            assert_eq!(without_overflow(&bytes), expected, "{}", original["id"]);
            let expected_facade = match expected {
                None => crate::StrictAsciiOutcome::UnicodeFailure,
                Some(false) => crate::StrictAsciiOutcome::NonAscii,
                Some(true) => crate::StrictAsciiOutcome::Ascii(
                    row["decoded_codepoints"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|p| char::from(p.as_u64().unwrap() as u8))
                        .collect(),
                ),
            };
            assert_eq!(
                crate::strict_idna_parameter(&bytes),
                Ok(expected_facade),
                "facade {}",
                original["id"]
            );
        }
        assert_eq!(ids.len(), 488);
    }
    #[test]
    fn native_nfkc_matches_every_official_unicode_3_2_sequence() {
        let mut executed = 0;
        for line in include_str!("../tests/NormalizationTest-3.2.0.txt").lines() {
            if line.is_empty() || line.starts_with(['#', '@']) {
                continue;
            }
            let fields: Vec<Vec<u32>> = line
                .split('#')
                .next()
                .unwrap()
                .split(';')
                .take(5)
                .map(|f| {
                    f.split_whitespace()
                        .map(|s| u32::from_str_radix(s, 16).unwrap())
                        .collect()
                })
                .collect();
            assert_eq!(fields.len(), 5);
            for input in &fields {
                assert_eq!(nfkc(input), fields[3], "{line}");
                executed += 1;
            }
        }
        assert_eq!(executed, 84960);
    }
    #[test]
    fn strict_idna_complete_source_ace_bytes_and_outcomes_for_every_codepoint() {
        use std::io::Read;
        let mut raw = Vec::new();
        flate2::read::ZlibDecoder::new(
            &include_bytes!("../tests/idna-all-points-source.bin.zlib")[..],
        )
        .read_to_end(&mut raw)
        .unwrap();
        let mut at = 0;
        let mut counts = [0usize; 3];
        for c in 0..0x110000 {
            let length = usize::from(u16::from_be_bytes(raw[at..at + 2].try_into().unwrap()));
            let expected = raw[at + 2];
            at += 3;
            let source = &raw[at..at + length];
            at += length;
            assert_eq!(
                encode(&[c]).unwrap(),
                source,
                "source ACE encoding U+{c:04x}"
            );
            let mut label = b"xn--".to_vec();
            label.extend_from_slice(source);
            let actual = match without_overflow(&label) {
                None => 0,
                Some(true) => 1,
                Some(false) => 2,
            };
            assert_eq!(actual, expected, "source strict IDNA U+{c:04x}");
            counts[usize::from(actual)] += 1;
        }
        assert_eq!(at, raw.len());
        assert_eq!(counts, [145535, 0, 968577]);
    }
}
