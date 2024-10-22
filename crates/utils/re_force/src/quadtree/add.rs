use super::{indexer::Indexer, LeafNode, Node, Position, Quadtree};

#[inline(always)]
fn leaf<T: Position>(value: T) -> Box<Node<T>> {
    Box::new(Node::Leaf(LeafNode::new(value)))
}

impl<P: Position> Quadtree<P> {
    pub fn add(&mut self, value: P) {
        self.cover(&value);

        let node = self.root.as_mut();

        let Some(mut node) = node else {
            self.root = Some(leaf(value));
            return;
        };

        let x = value.x();
        let y = value.y();
        let mut ix = Indexer::with_extent([self.x0, self.y0], [self.x1, self.y1]);

        loop {
            match node.as_mut() {
                Node::Internal(ref mut parent) => {
                    let i = ix.get_and_descend(x, y);
                    if let Some(ref mut n) = parent[i] {
                        node = n;
                    } else {
                        parent[i] = Some(leaf(value));
                        return;
                    }
                }
                // The new point coincides with the existing point.
                Node::Leaf(ref mut leaf) if x == leaf.data.x() && y == leaf.data.y() => {
                    let xp = leaf.data.x();
                    let yp = leaf.data.y();

                    if (x == xp) && (y == yp) {
                        leaf.insert(value);
                        return;
                    }

                    return;
                }
                old_leaf @ Node::Leaf(_) => {
                    let inner =
                        std::mem::replace(old_leaf, Node::Internal([None, None, None, None]));
                    if let Node::Leaf(inner) = inner {
                        let xp = inner.data.x();
                        let yp = inner.data.y();

                        let mut new_internal = old_leaf;

                        loop {
                            let Node::Internal(ref mut parent) = new_internal else {
                                unreachable!()
                            };

                            let j = ix.get(xp, yp);
                            let i = ix.get_and_descend(x, y);

                            debug_assert!(i < 4);
                            debug_assert!(j < 4);

                            if i != j {
                                parent[i] = Some(leaf(value));
                                parent[j] = Some(Box::new(Node::Leaf(inner)));
                                return;
                            }

                            parent[i] = Some(Box::new(Node::Internal([None, None, None, None])));
                            new_internal = parent[i].as_mut().unwrap();
                        }
                    }
                    unreachable!()
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn creates_a_new_point_and_adds_it_to_the_quadtree() {
        let mut q = Quadtree::default();

        q.add([0., 0.]);
        assert_eq!(q.root().unwrap().leaf().unwrap().data, [0., 0.]);

        q.add([0.9, 0.9]);
        assert!(matches!(
            q.root().unwrap().children().unwrap(),
            [
                Some(&Node::Leaf(LeafNode { data: [0., 0.], .. })),
                None,
                None,
                Some(&Node::Leaf(LeafNode {
                    data: [0.9, 0.9],
                    ..
                })),
            ]
        ));

        q.add([0.9, 0.0]);
        assert!(matches!(
            q.root().unwrap().children().unwrap(),
            [
                Some(&Node::Leaf(LeafNode { data: [0., 0.], .. })),
                Some(&Node::Leaf(LeafNode {
                    data: [0.9, 0.],
                    ..
                })),
                None,
                Some(&Node::Leaf(LeafNode {
                    data: [0.9, 0.9],
                    ..
                })),
            ]
        ));

        q.add([0., 0.9]);
        assert!(matches!(
            q.root().unwrap().children().unwrap(),
            [
                Some(&Node::Leaf(LeafNode { data: [0., 0.], .. })),
                Some(&Node::Leaf(LeafNode {
                    data: [0.9, 0.],
                    ..
                })),
                Some(&Node::Leaf(LeafNode {
                    data: [0.0, 0.9],
                    ..
                })),
                Some(&Node::Leaf(LeafNode {
                    data: [0.9, 0.9],
                    ..
                })),
            ]
        ));

        q.add([0.4, 0.4]);
        assert!(matches!(
            q.root().unwrap().children().unwrap(),
            [
                Some(&Node::Internal(_)),
                Some(&Node::Leaf(LeafNode {
                    data: [0.9, 0.],
                    ..
                })),
                Some(&Node::Leaf(LeafNode {
                    data: [0.0, 0.9],
                    ..
                })),
                Some(&Node::Leaf(LeafNode {
                    data: [0.9, 0.9],
                    ..
                })),
            ]
        ));
        assert!(matches!(
            q.root().unwrap().children().unwrap()[0].unwrap().children(),
            Some([
                Some(&Node::Leaf(LeafNode { data: [0., 0.], .. })),
                None,
                None,
                Some(&Node::Leaf(LeafNode {
                    data: [0.4, 0.4],
                    ..
                })),
            ])
        ));
    }

    #[test]
    fn handles_points_being_on_the_perimeter_of_the_quadtree_bounds() {
        let mut q = Quadtree::with_extent([0., 0.], [1., 1.]);
        q.add([0., 0.]);
        assert!(matches!(
            q.root(),
            Some(&Node::Leaf(LeafNode { data: [0., 0.], .. }))
        ));

        q.add([1., 1.]);
        assert!(matches!(
            q.root().unwrap().children().unwrap(),
            [
                Some(&Node::Leaf(LeafNode { data: [0., 0.], .. })),
                None,
                None,
                Some(&Node::Leaf(LeafNode { data: [1., 1.], .. })),
            ]
        ));

        q.add([1., 0.]);
        assert!(matches!(
            q.root().unwrap().children().unwrap(),
            [
                Some(&Node::Leaf(LeafNode { data: [0., 0.], .. })),
                Some(&Node::Leaf(LeafNode { data: [1., 0.], .. })),
                None,
                Some(&Node::Leaf(LeafNode { data: [1., 1.], .. })),
            ]
        ));

        q.add([0., 1.]);
        assert!(matches!(
            q.root().unwrap().children().unwrap(),
            [
                Some(&Node::Leaf(LeafNode { data: [0., 0.], .. })),
                Some(&Node::Leaf(LeafNode { data: [1., 0.], .. })),
                Some(&Node::Leaf(LeafNode { data: [0., 1.], .. })),
                Some(&Node::Leaf(LeafNode { data: [1., 1.], .. })),
            ]
        ));
    }

    #[test]
    fn handles_points_being_to_the_left_of_quadtree_bounds() {
        let mut q = Quadtree::with_extent([0., 0.], [2., 2.]);
        q.add([-1., 1.]);
        assert_eq!(dbg!(q).extent(), ([-4., 0.], [4., 8.]));
    }

    #[test]
    fn handles_coincident_points_by_creating_linked_list() {
        let mut q = Quadtree::with_extent([0., 0.], [1., 1.]);
        q.add([0., 0.]);
        assert!(matches!(
            q.root().unwrap(),
            &Node::Leaf(LeafNode { data: [0., 0.], .. })
        ));

        q.add([1., 0.]);
        assert!(matches!(
            q.root().unwrap().children().unwrap(),
            [
                Some(&Node::Leaf(LeafNode { data: [0., 0.], .. })),
                Some(&Node::Leaf(LeafNode { data: [1., 0.], .. })),
                None,
                None,
            ]
        ));

        q.add([0., 1.]);
        assert!(matches!(
            q.root().unwrap().children().unwrap(),
            [
                Some(&Node::Leaf(LeafNode { data: [0., 0.], .. })),
                Some(&Node::Leaf(LeafNode { data: [1., 0.], .. })),
                Some(&Node::Leaf(LeafNode { data: [0., 1.], .. })),
                None,
            ]
        ));

        q.add([0., 1.]);
        assert!(matches!(
            q.root().unwrap().children().unwrap(),
            [
                Some(&Node::Leaf(LeafNode { data: [0., 0.], .. })),
                Some(&Node::Leaf(LeafNode { data: [1., 0.], .. })),
                Some(&Node::Leaf(LeafNode { data: [0., 1.], .. })),
                None,
            ]
        ));
        assert_eq!(
            q.root().unwrap().children().unwrap()[2]
                .unwrap()
                .leaf()
                .unwrap()
                .iter()
                .collect::<Vec<_>>(),
            vec![&[0., 1.], &[0., 1.]],
        )
    }

    #[test]
    fn trivial_bounds_for_first_point() {
        let mut q = Quadtree::default();
        q.add([1.0, 2.0]);
        assert_eq!(q.extent(), ([1.0, 2.0], [2.0, 3.0]));
        assert!(
            matches!(q.root().unwrap(), Node::Leaf(leaf) if leaf.data.x() == 1.0 && leaf.data.y() == 2.0)
        );
    }
}
