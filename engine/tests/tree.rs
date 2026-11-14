//! The JSON form of the tree, held by properties over generated trees: a
//! tree round-trips, and a node that is not of the form is refused.

mod common;

use proptest::prelude::*;
use serde_json::{Value, json};
use trapperkeeper_engine::expr::Expr;

use common::expr;

proptest! {
    // No regression file, so a failure leaves nothing untracked in the tree.
    // The failing seed is in the output.
    #![proptest_config(ProptestConfig {
        failure_persistence: None,
        .. ProptestConfig::default()
    })]

    /// A generated tree writes to JSON and reads back equal.
    #[test]
    fn a_tree_round_trips_through_json(tree in expr()) {
        let json = serde_json::to_string(&tree).expect("a tree serializes");
        let back: Expr = serde_json::from_str(&json).expect("the JSON parses");
        // The floats compare exactly: serde_json prints the shortest text
        // that names the f64, and its float_roundtrip feature parses that
        // text to the nearest f64.
        prop_assert_eq!(back, tree);
    }

    /// A node whose key names no kind is refused.
    #[test]
    fn a_node_with_an_unknown_key_is_refused(tree in expr()) {
        // The tree stands under a `not`, so the node is an object with a
        // key also when the tree is `None`, which is a bare string.
        let node = serde_json::to_value(Expr::Not(Box::new(tree))).expect("a tree serializes");
        let misspelled = json!({ "nott": node["not"] });
        prop_assert!(serde_json::from_value::<Expr>(misspelled).is_err());
    }

    /// A node with a property that its kind does not have is refused.
    #[test]
    fn a_node_with_an_extra_property_is_refused((test, then, otherwise) in (expr(), expr(), expr())) {
        let (test, then, otherwise) = (Box::new(test), Box::new(then), Box::new(otherwise));
        let conditional = Expr::If { test, then, otherwise };
        let mut node = serde_json::to_value(conditional).expect("a tree serializes");
        // The enum denies unknown properties. Without that attribute serde
        // would skip the fourth property of this `if`.
        node["if"]["extra"] = Value::Null;
        prop_assert!(serde_json::from_value::<Expr>(node).is_err());
    }
}
