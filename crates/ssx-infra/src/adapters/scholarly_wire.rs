//! Engine-local request construction only; no factories or decoder fallback.
use super::scholarly_word::WORD;
use url::Url;
fn whitespace(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}
fn word(c: char) -> bool {
    let c = u32::from(c);
    WORD.binary_search_by(|&(start, end)| {
        if end < c {
            std::cmp::Ordering::Less
        } else if start > c {
            std::cmp::Ordering::Greater
        } else {
            std::cmp::Ordering::Equal
        }
    })
    .is_ok()
}
/// Python urllib.quote safe='/' versus HTTPX query parameter quote_plus.
fn quote(value: &str, plus: bool) -> String {
    let mut output = String::new();
    const HEX: &[u8] = b"0123456789ABCDEF";
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) || (!plus && byte == b'/') {
            output.push(char::from(byte));
        } else if plus && byte == b' ' {
            output.push('+');
        } else {
            output.push('%');
            output.push(char::from(HEX[usize::from(byte >> 4)]));
            output.push(char::from(HEX[usize::from(byte & 15)]));
        }
    }
    output
}
pub fn arxiv_query(query: &str) -> String {
    let stripped = query.trim_matches(whitespace);
    let mut count = 0;
    let mut plain = true;
    let mut boolean = false;
    for term in stripped.split(whitespace).filter(|t| !t.is_empty()) {
        count += 1;
        plain &= term.chars().all(|c| c == '-' || word(c));
        boolean |= ["AND", "OR", "ANDNOT"]
            .iter()
            .any(|operator| term.eq_ignore_ascii_case(operator));
    }
    if count >= 2 && plain && !boolean {
        format!("all:\"{stripped}\"")
    } else {
        format!("all:{query}")
    }
}
fn parameters(base: &str, pairs: &[(&str, &str)]) -> Result<String, url::ParseError> {
    let mut url = Url::parse(base)?;
    let query = pairs
        .iter()
        .map(|(k, v)| format!("{}={}", quote(k, true), quote(v, true)))
        .collect::<Vec<_>>()
        .join("&");
    url.set_query(Some(&query));
    Ok(url.into())
}
pub fn arxiv_url(base: &str, query: &str, limit: usize) -> Result<String, url::ParseError> {
    parameters(
        base,
        &[
            ("search_query", &arxiv_query(query)),
            ("start", "0"),
            ("max_results", &limit.to_string()),
            ("sortBy", "relevance"),
            ("sortOrder", "descending"),
        ],
    )
}
pub fn semantic_scholar_url(
    base: &str,
    query: &str,
    limit: usize,
) -> Result<String, url::ParseError> {
    parameters(
        base,
        &[
            ("query", query),
            ("limit", &limit.to_string()),
            (
                "fields",
                "title,url,abstract,citationCount,publicationDate,externalIds,authors",
            ),
        ],
    )
}
/// Actual OpenAlex validates dates/time_range before quota admission. The
/// enclosing service's relative-window/proof policy remains authoritative.
pub fn openalex_preflight(
    request: &ssx_domain::SearchRequest,
) -> Result<ssx_application::filters::PublicationBounds, String> {
    let bounds = ssx_application::filters::PublicationBounds::new(
        request.date_from.as_deref(),
        request.date_to.as_deref(),
    )
    .map_err(|error| error.message)?;
    if request.time_range.as_deref().is_some_and(|range| {
        !range.is_empty() && ssx_application::filters::time_range_days(range).is_none()
    }) {
        return Err("time_range must be day, week, month, or year".into());
    }
    Ok(bounds)
}
/// Caller validates inclusive date bounds before admission. This function only
/// preserves the source query order/quote mode and never invents an anchor.
pub fn openalex_url(
    base: &str,
    query: &str,
    limit: usize,
    lower: Option<&str>,
    upper: Option<&str>,
) -> String {
    let mut url = format!(
        "{base}/works?search={}&per_page={limit}",
        quote(query, false)
    );
    let mut filters = Vec::new();
    if let Some(date) = lower {
        filters.push(format!("from_publication_date:{date}"));
    }
    if let Some(date) = upper {
        filters.push(format!("to_publication_date:{date}"));
    }
    if !filters.is_empty() {
        url.push_str("&filter=");
        url.push_str(&quote(&filters.join(","), false));
    }
    url
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn historical_first_requests_and_openalex_preflight_errors_remain_source_exact() {
        let mut count = 0;
        let mut invalid = 0;
        for raw in [
            include_str!("../../../../docs/rust-rewrite/a06/source-initial.json"),
            include_str!("../../../../docs/rust-rewrite/a06/source-regressions.json"),
            include_str!("../../../../docs/rust-rewrite/a06/source-shared.json"),
        ] {
            let corpus: ssx_json::Value = ssx_json::from_str(raw).unwrap();
            for row in corpus["records"].as_array().unwrap() {
                let engine = row["engine"].as_str().unwrap();
                let query = row["query"].as_str().unwrap();
                let cfg = &row["config"];
                let limit = cfg["max_results"]
                    .as_u64()
                    .unwrap_or(if engine == "openalex" { 10 } else { 5 })
                    as usize;
                let mut request = ssx_domain::SearchRequest::default();
                request.date_from = row["params"]["date_from"].as_str().map(str::to_owned);
                request.date_to = row["params"]["date_to"].as_str().map(str::to_owned);
                request.time_range = row["params"]["time_range"].as_str().map(str::to_owned);
                if engine == "openalex"
                    && let Err(message) = openalex_preflight(&request)
                {
                    assert_eq!(message, row["output"]["error_message"].as_str().unwrap());
                    assert!(row["requests"].as_array().unwrap().is_empty());
                    invalid += 1;
                    continue;
                }
                let Some(expected) = row["requests"].as_array().unwrap().first() else {
                    continue;
                };
                let actual = match engine {
                    "arxiv" => arxiv_url(
                        cfg["base_url"]
                            .as_str()
                            .unwrap_or("https://export.arxiv.org/api/query"),
                        query,
                        limit,
                    )
                    .unwrap(),
                    "semanticscholar" => semantic_scholar_url(
                        cfg["base_url"]
                            .as_str()
                            .unwrap_or("https://api.semanticscholar.org/graph/v1/paper/search"),
                        query,
                        limit,
                    )
                    .unwrap(),
                    "openalex" => openalex_url(
                        cfg["base_url"]
                            .as_str()
                            .unwrap_or("https://api.openalex.org"),
                        query,
                        limit,
                        request.date_from.as_deref(),
                        request.date_to.as_deref(),
                    ),
                    _ => unreachable!(),
                };
                assert_eq!(actual, expected["url"].as_str().unwrap(), "{}", row["id"]);
                count += 1;
            }
        }
        assert_eq!(invalid, 9);
        assert_eq!(count, 106);
    }
    #[test]
    fn exhaustive_unicode_word_ranges_and_actual_source_request_urls() {
        let fixture: ssx_json::Value = ssx_json::from_str(include_str!(
            "../../../../docs/rust-rewrite/a06/source-wire.json"
        ))
        .unwrap();
        let ranges = fixture["word_predicate"]["ranges"].as_array().unwrap();
        assert_eq!(ranges.len(), WORD.len());
        for (actual, expected) in WORD.iter().zip(ranges) {
            assert_eq!(
                (actual.0 as u64, actual.1 as u64),
                (expected[0].as_u64().unwrap(), expected[1].as_u64().unwrap())
            );
        }
        // Every Unicode scalar is checked against the captured exhaustive oracle,
        // including combining marks that Rust Alphabetic/regex \\w may classify differently.
        let mut range = 0;
        for point in 0..=0x10ffff {
            while range < WORD.len() && WORD[range].1 < point {
                range += 1;
            }
            if let Some(c) = char::from_u32(point) {
                assert_eq!(word(c), range < WORD.len() && WORD[range].0 <= point);
            }
        }
        let mut count = 0;
        for row in fixture["records"].as_array().unwrap().iter().take(63) {
            let query = row["query"].as_str().unwrap();
            let actual = match row["engine"].as_str().unwrap() {
                "arxiv" => arxiv_url("https://export.arxiv.org/api/query", query, 5).unwrap(),
                "semanticscholar" => semantic_scholar_url(
                    "https://api.semanticscholar.org/graph/v1/paper/search",
                    query,
                    5,
                )
                .unwrap(),
                "openalex" => openalex_url("https://api.openalex.org", query, 10, None, None),
                _ => unreachable!(),
            };
            assert_eq!(
                actual,
                row["requests"][0]["url"].as_str().unwrap(),
                "{}",
                row["id"]
            );
            count += 1;
        }
        assert_eq!(count, 63);
    }
}
