//! The properties in this file check the JSON form of the tree over
//! generated trees. A tree round-trips, and a node with an unknown key or an
//! extra property is refused.

mod common;

use proptest::prelude::*;
use serde_json::{Value, json};
use trapperkeeper_engine::expr::Expr;

use common::expr;

proptest! {
    // `failure_persistence: None` turns off proptest's regression file, so a
    // failing run writes no untracked file into the working tree. The failure
    // output shows the shrunk failing input.
    #![proptest_config(ProptestConfig {
        failure_persistence: None,
        .. ProptestConfig::default()
    })]

    /// A generated tree writes to JSON and reads back equal.
    #[test]
    fn a_tree_round_trips_through_json(tree in expr()) {
        let json = serde_json::to_string(&tree).expect("a tree serializes");
        let back: Expr = serde_json::from_str(&json).expect("the JSON parses");
        // The floats compare exactly, because serde_json prints the shortest
        // text that names an f64, and its float_roundtrip feature parses that
        // text to the nearest f64.
        prop_assert_eq!(back, tree);
    }

    /// A node whose key names no kind is refused.
    #[test]
    fn a_node_with_an_unknown_key_is_refused(tree in expr()) {
        // The tree stands under a `not`, so the node is an object with a
        // key also when the tree is `None`, which is a bare string.
        let node = serde_json::to_value(Expr::Not(Box::new(tree))).expect("a tree serializes");
        // The operand under `nott` is the JSON of a valid tree, so only the
        // unknown key can fail the parse.
        let misspelled = json!({ "nott": node["not"] });
        prop_assert!(serde_json::from_value::<Expr>(misspelled).is_err());
    }

    /// A node with a property that its kind does not have is refused.
    #[test]
    fn a_node_with_an_extra_property_is_refused((test, then, otherwise) in (expr(), expr(), expr())) {
        let (test, then, otherwise) = (Box::new(test), Box::new(then), Box::new(otherwise));
        // An `if` is a kind whose JSON is an object of named properties, so
        // one more property can be added to it.
        let conditional = Expr::If { test, then, otherwise };
        let mut node = serde_json::to_value(conditional).expect("a tree serializes");
        // `deny_unknown_fields` on `Expr` makes serde refuse the property.
        // Without it serde would skip the fourth property of this `if`.
        node["if"]["extra"] = Value::Null;
        prop_assert!(serde_json::from_value::<Expr>(node).is_err());
    }
}
