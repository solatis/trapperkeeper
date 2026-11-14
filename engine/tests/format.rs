//! The JSON form of a rule, pinned by one literal document: it parses to
//! the expected rule, writes back to the same bytes, and an unknown property
//! or a misspelled word is refused. `tree.rs` pins the keys of the tree, and
//! this file pins the properties of a rule and their order.

use std::collections::BTreeMap;

use trapperkeeper_engine::{Action, Expr, Rule, Segment, validate};

/// One rule with every field set, as `serde_json::to_string_pretty` writes
/// it. Its expression is `key.val1 > key.val2 and key.val3 > 0`.
const RULE: &str = r#"{
  "rule_id": "01937b2e-7c00-7000-8000-000000000001",
  "rule_title": "Values out of order in production",
  "action": "throw",
  "sample_rate": 0.25,
  "scope": {
    "env": "prod"
  },
  "expression": {
    "and": [
      {
        "gt": [
          {
            "field": [
              "key",
              "val1"
            ]
          },
          {
            "field": [
              "key",
              "val2"
            ]
          }
        ]
      },
      {
        "gt": [
          {
            "field": [
              "key",
              "val3"
            ]
          },
          {
            "int": 0
          }
        ]
      }
    ]
  }
}"#;

/// The rule `RULE` spells.
fn rule() -> Rule {
    let field = |k: &str| {
        let path = ["key", k].map(|k| Segment::Key(k.to_owned()));
        Box::new(Expr::Field(path.to_vec()))
    };
    Rule {
        rule_id: "01937b2e-7c00-7000-8000-000000000001"
            .parse()
            .expect("a UUID"),
        rule_title: Some("Values out of order in production".to_owned()),
        action: Action::Throw,
        sample_rate: 0.25,
        scope: BTreeMap::from([("env".to_owned(), "prod".to_owned())]),
        expression: Expr::And(vec![
            Expr::Gt(field("val1"), field("val2")),
            Expr::Gt(field("val3"), Box::new(Expr::Int(0))),
        ]),
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
        ("\"expression\"", "\"expresion\""),
        ("\"throw\"", "\"thow\""),
        ("\"gt\"", "\"gtt\""),
    ];
    for (word, typo) in misspelled {
        assert!(RULE.contains(word), "{word} is not in the document");
        let document = RULE.replace(word, typo);
        let parsed = serde_json::from_str::<Rule>(&document);
        assert!(parsed.is_err(), "{typo} was accepted");
    }
}
