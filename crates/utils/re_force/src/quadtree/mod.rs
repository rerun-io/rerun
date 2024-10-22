mod add;
mod cover;
mod indexer;
mod position;

pub use position::Position;

#[derive(Clone, Debug, PartialEq, Eq)]
struct LeafNode<T: Position> {
    data: T,
    next: Option<Box<LeafNode<T>>>,
}

impl<'a, T: Position> LeafNode<T> {
    fn new(data: T) -> Self {
        Self { data, next: None }
    }

    fn insert(&mut self, data: T) {
        let mut node = self;
        loop {
            match node.next {
                Some(ref mut next) => {
                    node = next;
                }
                None => {
                    node.next = Some(Box::new(LeafNode::new(data)));
                    return;
                }
            }
        }
    }

    fn iter(&self) -> impl Iterator<Item = &T> {
        LeafListIterator { next: Some(self) }
    }
}

struct LeafListIterator<'a, T: Position> {
    next: Option<&'a LeafNode<T>>,
}

impl<'a, T: Position> Iterator for LeafListIterator<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.next;
        self.next = next.and_then(|node| node.next.as_deref());
        next.map(|node| &node.data)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node<T: Position> {
    Leaf(LeafNode<T>),
    Internal([Option<Box<Node<T>>>; 4]),
}

impl<T: Position> Node<T> {
    fn leaf(&self) -> Option<&LeafNode<T>> {
        match self {
            Node::Leaf(leaf) => Some(leaf),
            _ => None,
        }
    }

    fn children(&self) -> Option<[Option<&Node<T>>; 4]> {
        match self {
            Node::Leaf(_) => None,
            Node::Internal(children) => Some([
                children[0].as_deref(),
                children[1].as_deref(),
                children[2].as_deref(),
                children[3].as_deref(),
            ]),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Quadtree<T: Position> {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    root: Option<Box<Node<T>>>,
}

impl<T: Position> Default for Quadtree<T> {
    fn default() -> Self {
        Self {
            x0: f32::NAN,
            y0: f32::NAN,
            x1: f32::NAN,
            y1: f32::NAN,
            root: None,
        }
    }
}

impl<P: Position> Quadtree<P> {
    pub fn with_extent(min: [f32; 2], max: [f32; 2]) -> Self {
        Self {
            x0: min[0],
            y0: min[1],
            x1: max[0],
            y1: max[1],
            root: None,
        }
    }

    pub fn from_nodes(nodes: &[P]) -> Self {
        let tree = Self {
            x0: f32::NAN,
            y0: f32::NAN,
            x1: f32::NAN,
            y1: f32::NAN,
            root: None,
        };

        if nodes.is_empty() {
            return tree;
        }

        // tree.add_all(nodes);

        tree
    }

    pub fn extent(&self) -> ([f32; 2], [f32; 2]) {
        ([self.x0, self.y0], [self.x1, self.y1])
    }

    pub fn root(&self) -> Option<&Node<P>> {
        self.root.as_ref().map(|node| &**node)
    }
}
