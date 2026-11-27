//! The tree of the paths that a rule set reads, and the numbers of its fields.

use crate::expr::{Expr, Segment};

/// The paths that a rule set reads, as a tree whose edges are the segments of
/// the paths. A node that ends a path carries the number of its field, which
/// is the place of the path in `paths`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldTree {
    // The tree that an adapter, the code that describes the columns of a
    // source, walks to look up a prefix that two paths share once.
    root: FieldNode,
    // The paths in field order, which is the order of the columns that a
    // caller hands over.
    paths: Vec<Vec<Segment>>,
}

/// One node of a `FieldTree`: the segments that lead on from here, and the
/// number of the field whose path ends here, if one does.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FieldNode {
    children: Vec<(Segment, FieldNode)>,
    field: Option<usize>,
}

impl FieldTree {
    /// The tree of every path that `exprs` read, with the fields numbered in
    /// the order of a depth-first walk.
    pub(crate) fn of<'e>(exprs: impl IntoIterator<Item = &'e Expr>) -> Self {
        // The function inserts every path into the tree and then numbers the
        // nodes that end a path.
        //
        // 1. It walks every expression and inserts each path that it reads.
        //    The children of a node stay in the order in which the rules first
        //    read them, and two reads of one path end at one node, which is
        //    one field.
        // 2. It numbers the nodes that end a path in a depth-first walk and
        //    records each path in that order. Numbering the finished tree
        //    makes the order of the fields the order of a walk over it, so an
        //    adapter that walks the tree meets the fields in the order of the
        //    columns that the caller hands over.

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

    /// The number of the field with `path`.
    ///
    /// # Panics
    ///
    /// Panics when no expression of the tree reads `path`.
    pub(crate) fn field_of(&self, path: &[Segment]) -> usize {
        path.iter()
            .try_fold(&self.root, |node, s| node.child(s))
            .and_then(|node| node.field)
            // The planner asks only for the paths of the expressions that
            // built the tree, so a missing path is a bug of the planner.
            .expect("the tree holds every path that its expressions read")
    }

    /// The paths in field order.
    #[must_use]
    pub fn paths(&self) -> &[Vec<Segment>] {
        &self.paths
    }

    /// The root of the tree, whose children are the first segments of the
    /// paths.
    #[must_use]
    pub fn root(&self) -> &FieldNode {
        &self.root
    }
}

impl FieldNode {
    /// The segments that lead on from this node, each with the node it leads
    /// to.
    #[must_use]
    pub fn children(&self) -> &[(Segment, FieldNode)] {
        &self.children
    }

    /// The number of the field whose path ends at this node.
    #[must_use]
    pub fn field(&self) -> Option<usize> {
        self.field
    }

    /// The node that `segment` leads to from this node.
    fn child(&self, segment: &Segment) -> Option<&FieldNode> {
        self.children
            .iter()
            .find(|(s, _)| s == segment)
            .map(|(_, node)| node)
    }

    /// Inserts `path` below this node, and marks the node where it ends.
    fn insert(&mut self, path: &[Segment]) {
        let Some((first, rest)) = path.split_first() else {
            // `number` replaces this placeholder with the field's number.
            self.field = Some(0);
            return;
        };
        let at = self
            .children
            .iter()
            .position(|(s, _)| s == first)
            // A new child goes last, so the children keep the order in which
            // the rules first read them.
            .unwrap_or_else(|| {
                self.children.push((first.clone(), FieldNode::default()));
                self.children.len() - 1
            });
        self.children[at].1.insert(rest);
    }

    /// Numbers the nodes from this one down that end a path, depth first,
    /// and appends their paths to `paths`. `prefix` is the path to this node.
    fn number(&mut self, prefix: &mut Vec<Segment>, paths: &mut Vec<Vec<Segment>>) {
        // A path that ends here takes its number before the longer paths
        // below it, as in a preorder walk.
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

/// The paths of the fields that `e` and the nodes below it read.
fn paths_in(e: &Expr) -> Vec<&[Segment]> {
    match e {
        Expr::Field(path) => vec![path],
        // An element of a quantifier is a leaf with no children, and its path
        // starts at the element, so it adds no field of the record.
        _ => e
            .children()
            .into_iter()
            .flat_map(|(_, child)| paths_in(child))
            .collect(),
    }
}
