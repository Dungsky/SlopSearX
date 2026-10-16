#[test]
fn source_ascii_substring_lowering_preimages_and_complete_controls() {
    let evidence: serde_json::Value =
        serde_json::from_str(include_str!("lower-ascii-source.json")).unwrap();
    assert_eq!(
        evidence["exhaustive_nonascii_preimages"],
        serde_json::json!([
            {"input":304,"lower_codepoints":[105,775]},
            {"input":8490,"lower_codepoints":[107]}
        ])
    );
    assert_eq!(evidence["empty_lower_preimages"], serde_json::json!([]));
    let rows = evidence["cases"].as_array().unwrap();
    assert_eq!(rows.len(), 2111);
    for row in rows {
        let points = row["points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|point| u32::try_from(point.as_u64().unwrap()).unwrap())
            .collect();
        let text = ssx_source_codecs::Text::from_codepoints(points).unwrap();
        assert_eq!(
            text.contains_ascii_lower(row["needle"].as_str().unwrap()),
            row["expected"].as_bool().unwrap(),
            "{row}"
        );
    }
}
