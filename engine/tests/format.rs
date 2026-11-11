//! The JSON form of a rule, pinned by one literal document: it parses to
//! the expected rule, writes back to the same bytes, and an unknown property
//! or a misspelled word is refused. The properties in `model.rs` cover
//! generated rules, and this file fixes the names the format uses.

use std::collections::BTreeMap;

use trapperkeeper_engine::{
    Action, Condition, FieldPath, FieldType, Group, Operator, Policy, Rule, Scalar, Segment, Value,
    validate,
};

/// One rule with every field set, as `serde_json::to_string_pretty` writes
/// it.
const RULE: &str = r#"{
  "rule_id": "01937b2e-7c00-7000-8000-000000000001",
  "rule_title": "Expensive items in production",
  "action": "drop",
  "sample_rate": 0.25,
  "scope": {
    "env": "prod"
  },
  "groups": [
    {
      "conditions": [
        {
          "path": [
            {
              "key": "order"
            },
            {
              "key": "items"
            },
            "*",
            {
              "key": "price"
            }
          ],
          "field_type": "float",
          "op": "gt",
          "value": 100,
          "on_missing": "skip",
          "on_coercion_failure": "fail"
        },
        {
          "path": [
            {
              "key": "order"
            },
            {
              "key": "currency"
            }
          ],
          "field_type": "string",
          "op": "not_in",
          "value": [
            "EUR",
            "USD"
          ],
          "on_missing": "match",
          "on_coercion_failure": "skip"
        }
      ]
    },
    {
      "conditions": [
        {
          "path": [
            {
              "key": "lines"
            },
            {
              "index": 0
            },
            {
              "key": "ratio"
            }
          ],
          "field_type": "float",
          "op": "le",
          "value": 0.5,
          "on_missing": "skip",
          "on_coercion_failure": "skip"
        }
      ]
    }
  ]
}"#;

/// The rule `RULE` spells.
fn rule() -> Rule {
    let key = |k: &str| Segment::Key(k.to_owned());
    Rule {
        rule_id: "01937b2e-7c00-7000-8000-000000000001"
            .parse()
            .expect("a UUID"),
        rule_title: Some("Expensive items in production".to_owned()),
        action: Action::Drop,
        sample_rate: 0.25,
        scope: BTreeMap::from([("env".to_owned(), "prod".to_owned())]),
        groups: vec![
            Group {
                conditions: vec![
                    Condition {
                        path: FieldPath(vec![
                            key("order"),
                            key("items"),
                            Segment::Wildcard,
                            key("price"),
                        ]),
                        field_type: FieldType::Float,
                        op: Operator::Gt,
                        value: Value::One(Scalar::Int(100)),
                        on_missing: Policy::Skip,
                        on_coercion_failure: Policy::Fail,
                    },
                    Condition {
                        path: FieldPath(vec![key("order"), key("currency")]),
                        field_type: FieldType::String,
                        op: Operator::NotIn,
                        value: Value::Many(vec![
                            Scalar::String("EUR".to_owned()),
                            Scalar::String("USD".to_owned()),
                        ]),
                        on_missing: Policy::Match,
                        on_coercion_failure: Policy::Skip,
                    },
                ],
            },
            Group {
                conditions: vec![Condition {
                    path: FieldPath(vec![key("lines"), Segment::Index(0), key("ratio")]),
                    field_type: FieldType::Float,
                    op: Operator::Le,
                    value: Value::One(Scalar::Float(0.5)),
                    on_missing: Policy::Skip,
                    on_coercion_failure: Policy::Skip,
                }],
            },
        ],
    }
}

#[test]
fn the_document_parses_writes_back_and_validates() {
    let parsed: Rule = serde_json::from_str(RULE).expect("the document parses");
    assert_eq!(parsed, rule());
    // Byte for byte, so the test also fixes the order of the properties.
    let written = serde_json::to_string_pretty(&parsed).expect("a rule serializes");
    assert_eq!(written, RULE);
    assert_eq!(validate(&parsed), Ok(()));
}

#[test]
fn an_unknown_property_and_a_misspelled_word_are_refused() {
    let misspelled = [
        ("\"on_missing\"", "\"on_mising\""),
        ("\"*\"", "\"wildcard\""),
        ("\"drop\"", "\"dorp\""),
    ];
    for (word, typo) in misspelled {
        assert!(RULE.contains(word), "{word} is not in the document");
        let document = RULE.replace(word, typo);
        let parsed = serde_json::from_str::<Rule>(&document);
        assert!(parsed.is_err(), "{typo} was accepted");
    }
}
