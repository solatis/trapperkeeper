//! Lists the fields that a rule set reads and gives each one a number. The
//! number is how the steps of a program refer to a field, and it is the
//! position of the field's column in the list a caller passes to `bind`.

use crate::expr::{Expr, Segment};

/// The fields that a rule set reads, as a tree of their paths.
///
/// A path such as `user.address.city` is a chain of segments, and each
/// segment is one edge of the tree. So `user.name` and `user.address.city`
/// share the edge `user`. The node at the end of a path carries the number
/// of its field. Fields are numbered in the order of a depth-first walk over
/// the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldTree {
    // An adapter, the code that finds the columns of a source such as a
    // DataFrame, walks this tree. It looks up `user` once for both fields
    // below it.
    root: FieldNode,
    // The path of each field, at the index of the field's number. A caller
    // passes its columns in this order.
    paths: Vec<Vec<Segment>>,
}

/// One node of a `FieldTree`: the edges to its children, and the number of
/// the field whose path ends at this node, if a path ends here.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FieldNode {
    children: Vec<(Segment, FieldNode)>,
    field: Option<usize>,
}

impl FieldTree {
    /// Builds the tree of every field that `exprs` read.
    pub(crate) fn of<'e>(exprs: impl IntoIterator<Item = &'e Expr>) -> Self {
        // The fields are numbered only after every path is in the tree.
        // Numbering while inserting would number fields in the order the
        // rules read them, and an adapter that walks the tree would then meet
        // the fields out of order.
        //
        // 1. Insert every path that the expressions read. A field that two
        //    rules read is one node, so it is one field. The children of a
        //    node stay in the order in which the rules first read them.
        // 2. Walk the finished tree depth first, number the nodes where a
        //    path ends, and record each path under its number.

        // 1. insert each path
        let mut root = FieldNode::default();
        for path in exprs.into_iter().flat_map(paths_in) {
            root.insert(path);
        }

        // 2. number the fields depth first
        let mut paths = Vec::new();
        root.number(&mut Vec::new(), &mut paths);
        Self { root, paths }
    }

    /// Returns the number of the field at `path`.
    ///
    /// # Panics
    ///
    /// Panics when no expression of the tree reads `path`.
    pub(crate) fn field_of(&self, path: &[Segment]) -> usize {
        path.iter()
            // Follows the path one segment at a time from the root, and
            // stops with `None` at a segment that has no edge.
            .try_fold(&self.root, |node, s| node.child(s))
            .and_then(|node| node.field)
            // The planner only asks for paths of the rules that built the
            // tree, so a missing path is a bug in the planner.
            .expect("the tree holds every path that its expressions read")
    }

    /// The path of each field, at the index of the field's number.
    #[must_use]
    pub fn paths(&self) -> &[Vec<Segment>] {
        &self.paths
    }

    /// The root of the tree. Its edges are the first segments of the paths,
    /// such as `user`.
    #[must_use]
    pub fn root(&self) -> &FieldNode {
        &self.root
    }
}

impl FieldNode {
    /// The edges from this node, each with the child it leads to.
    #[must_use]
    pub fn children(&self) -> &[(Segment, FieldNode)] {
        &self.children
    }

    /// The number of the field whose path ends at this node, or `None` when
    /// the node is only part of longer paths.
    #[must_use]
    pub fn field(&self) -> Option<usize> {
        self.field
    }

    /// Returns the child at the end of the edge `segment`, if there is one.
    fn child(&self, segment: &Segment) -> Option<&FieldNode> {
        self.children
            .iter()
            .find(|(s, _)| s == segment)
            .map(|(_, node)| node)
    }

    /// Adds `path` below this node, creating the nodes it lacks, and marks
    /// the node where it ends as a field.
    fn insert(&mut self, path: &[Segment]) {
        let Some((first, rest)) = path.split_first() else {
            // The path ends here. `Some(0)` only marks the node as a field,
            // and `number` writes the real number later.
            self.field = Some(0);
            return;
        };
        let at = self
            .children
            .iter()
            .position(|(s, _)| s == first)
            // A segment seen for the first time gets a new child at the end,
            // so the children stay in the order the rules first read them.
            .unwrap_or_else(|| {
                self.children.push((first.clone(), FieldNode::default()));
                self.children.len() - 1
            });
        self.children[at].1.insert(rest);
    }

    /// Numbers the fields at and below this node in depth-first order, and
    /// appends their paths to `paths`. `prefix` is the path from the root to
    /// this node.
    fn number(&mut self, prefix: &mut Vec<Segment>, paths: &mut Vec<Vec<Segment>>) {
        // A field at this node gets its number before the fields below it,
        // so `user` comes before `user.name`.
        if self.field.is_some() {
            self.field = Some(paths.len());
            paths.push(prefix.clone());
        }
        for (segment, child) in &mut self.children {
            prefix.push(segment.clone());
            child.number(prefix, paths);
            prefix.pop();
        }
    }
}

/// Returns the path of every field that `e` reads, at any depth.
fn paths_in(e: &Expr) -> Vec<&[Segment]> {
    match e {
        Expr::Field(path) => vec![path],
        // Every other expression gives the paths of its children. An
        // `Expr::Element`, the `x` in `any(x > 5 for x in xs)`, has no
        // children and so gives no path, which is right, because its path
        // starts at the list element and is not a field of the record.
        _ => e
            .children()
            .into_iter()
            .flat_map(|(_, child)| paths_in(child))
            .collect(),
    }
}
