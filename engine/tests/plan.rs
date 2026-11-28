//! Tests of the planner: the numbering of fields, the refusal of an
//! expression this version does not support, and the sharing of a step that
//! two rules both need.

mod common;

use proptest::collection::vec;
use proptest::prelude::*;
use proptest::sample::select;
use trapperkeeper_engine::Rule;
use trapperkeeper_engine::expr::{Expr, Segment};
use trapperkeeper_engine::plan::{FieldNode, Unsupported, plan};
use uuid::Uuid;

use common::{observing, rule_id};

/// Paths of one to three segments over two keys and two indexes, so that
/// generated paths often share a prefix.
fn path() -> impl Strategy<Value = Vec<Segment>> {
    let segment = select(vec![
        Segment::Key("a".into()),
        Segment::Key("b".into()),
        Segment::Index(0),
        Segment::Index(1),
    ]);
    vec(segment, 1..4)
}

/// Returns the field numbers at and below `node` in depth-first order: the
/// node's own field first, then the fields below each child, in the order the
/// node stores its children.
fn walk(node: &FieldNode) -> Vec<usize> {
    node.field()
        .into_iter()
        .chain(node.children().iter().flat_map(|(_, child)| walk(child)))
        .collect()
}

proptest! {
    // `failure_persistence: None` turns off proptest's regression file, so a
    // failing run writes no untracked file into the working tree.
    #![proptest_config(ProptestConfig {
        failure_persistence: None,
        .. ProptestConfig::default()
    })]

    /// Every path that a generated rule set reads appears once in `paths`, and
    /// a depth-first walk over the tree meets the fields in the order of their
    /// numbers.
    #[test]
    fn the_fields_are_numbered_in_the_order_of_a_walk(
        reads in vec((path(), any::<i64>()), 1..12),
        id in rule_id(),
    ) {
        // The test plans a generated rule set, then checks the list of paths
        // and the tree against the paths that the rules read.
        //
        // 1. It builds one rule per comparison of a field with an int, and
        //    plans the rules.
        // 2. It checks that `paths` holds each path that a rule reads once.
        // 3. It walks the tree depth first and checks that it meets the
        //    fields in the order of their numbers.

        // 1. plan the generated rules
        // The planner needs no unique ids, so all rules share one.
        let rules: Vec<Rule> = reads
            .iter()
            .map(|(p, i)| {
                let field = Box::new(Expr::Field(p.clone()));
                observing(id, Expr::Lt(field, Box::new(Expr::Int(*i))))
            })
            .collect();
        let program = plan(&rules).expect("a comparison of a field with an int plans");

        // 2. check that each path appears once
        let paths = program.fields().paths();
        // A `Segment` can be neither sorted nor hashed, so the test removes
        // duplicate paths with a plain list.
        let mut read: Vec<&Vec<Segment>> = Vec::new();
        for (p, _) in &reads {
            if !read.contains(&p) {
                read.push(p);
            }
        }
        // `paths` holds every path that a rule reads and no more paths than
        // there are, so it holds each one once.
        prop_assert_eq!(paths.len(), read.len());
        prop_assert!(read.iter().all(|p| paths.contains(p)));

        // 3. walk the tree and check the order
        let numbers = walk(program.fields().root());
        prop_assert_eq!(numbers, (0..paths.len()).collect::<Vec<_>>());
    }
}

/// `plan` refuses a rule that uses an expression this version does not
/// support, and the error names the rule's id and the expression's JSON
/// key.
#[test]
fn an_unsupported_kind_is_refused_with_its_rule() {
    let id = Uuid::from_u128(7);
    let sum = Expr::Add(
        Box::new(Expr::Field(vec![Segment::Key("x".into())])),
        Box::new(Expr::Int(1)),
    );
    // The first rule is fine, so the error must name the second rule, which
    // holds the arithmetic.
    let rules = [
        observing(Uuid::from_u128(1), Expr::Bool(true)),
        observing(id, Expr::Gt(Box::new(sum), Box::new(Expr::Int(2)))),
    ];
    assert_eq!(
        plan(&rules).err(),
        Some(Unsupported {
            rule_id: id,
            key: "add"
        })
    );
}

/// The planner refuses a list inside a membership list, with the key of the
/// inner list.
#[test]
fn a_list_inside_a_membership_list_is_refused() {
    let id = Uuid::from_u128(7);
    let inner = Expr::List(vec![Expr::Int(1)]);
    let rule = Expr::In(
        Box::new(Expr::Field(vec![Segment::Key("x".into())])),
        Box::new(Expr::List(vec![Expr::Int(2), inner])),
    );
    assert_eq!(
        plan(&[observing(id, rule)]).err(),
        Some(Unsupported {
            rule_id: id,
            key: "list"
        })
    );
}

/// The planner refuses a written list as the needle of a search, with the key
/// of the list, because list equality does not exist.
#[test]
fn a_written_list_as_the_needle_is_refused() {
    let id = Uuid::from_u128(7);
    let rule = Expr::In(
        Box::new(Expr::List(vec![Expr::Int(1), Expr::Int(2)])),
        Box::new(Expr::Field(vec![Segment::Key("tags".into())])),
    );
    assert_eq!(
        plan(&[observing(id, rule)]).err(),
        Some(Unsupported {
            rule_id: id,
            key: "list"
        })
    );
}

/// Two rules that make the same comparison share one step for it.
#[test]
fn rules_share_an_equal_comparison() {
    let gt = || {
        Expr::Gt(
            Box::new(Expr::Field(vec![Segment::Key("x".into())])),
            Box::new(Expr::Int(5)),
        )
    };
    // In the second rule the comparison sits under a `not`, so the test
    // also covers sharing of a step that is not the top of a rule.
    let rules = [
        observing(Uuid::from_u128(1), gt()),
        observing(Uuid::from_u128(2), Expr::Not(Box::new(gt()))),
    ];
    let program = plan(&rules).expect("the rules plan");
    // The program shows its steps only through `Debug`, where each
    // comparison step prints as `Compare`.
    assert_eq!(format!("{program:?}").matches("Compare").count(), 1);
}
