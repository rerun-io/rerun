mod position;
pub use position::Position;

#[derive(Debug)]
pub enum Node<T: Position> {
    Leaf { data: T },
    Internal { children: [Option<Box<Node<T>>>; 4] },
}

#[derive(Debug)]
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

impl<T: Position> Quadtree<T> {
    pub fn with_nodes(nodes: &[T]) -> Self {
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

    pub fn cover(&mut self, value: &impl Position) {
        let x = value.x();
        let y = value.y();

        debug_assert!(!f32::is_nan(x));
        debug_assert!(!f32::is_nan(y));

        if f32::is_nan(self.x0) {
            self.x0 = x.floor();
            self.x1 = self.x0 + 1.0;
            self.y0 = y.floor();
            self.y1 = self.y0 + 1.0;
        } else {
            // Otherwise, double repeatedly to cover.
            let mut z = if (self.x1 - self.x0).is_sign_positive() {
                1.0
            } else {
                0.0
            };

            while self.x0 > x || x >= self.x1 || self.y0 > y || y >= self.y1 {
                let i = ((y < self.y0) as usize) << 1 | ((x < self.x0) as usize);
                let mut children = [None, None, None, None];
                children[i] = self.root.take();
                self.root = Some(Box::new(Node::Internal { children }));
                z *= 2.0;
                match i {
                    0 => {
                        self.x1 = self.x0 + z;
                        self.y1 = self.y0 + z;
                    }
                    1 => {
                        self.x0 = self.x1 - z;
                        self.y1 = self.y0 + z;
                    }
                    2 => {
                        self.x1 = self.x0 + z;
                        self.y0 = self.y1 - z;
                    }
                    3 => {
                        self.x0 = self.x1 - z;
                        self.y0 = self.y1 - z;
                    }
                    _ => unreachable!(),
                }
            }
        }
    }

    pub fn add(mut self, value: T) -> Self {
        self.cover(&value);

        if self.root.is_none() {
            self.root = Some(Box::new(Node::Leaf { data: value }));
            return self;
        }

        todo!();

        self
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn cover() {
        let mut tree = Quadtree::<[f32; 2]>::default();
        assert!(tree.x0.is_nan());

        tree.cover(&[0.5, 0.5]);
        assert_eq!(tree.x0, 0.0);
        assert_eq!(tree.y0, 0.0);
        assert_eq!(tree.x1, 1.0);
        assert_eq!(tree.y1, 1.0);

        tree.cover(&[1.5, 1.5]);
        assert_eq!(tree.x0, 0.0);
        assert_eq!(tree.y0, 0.0);
        assert_eq!(tree.x1, 2.0);
        assert_eq!(tree.y1, 2.0);
    }

    #[test]
    fn add() {
        let tree = Quadtree::<[f32; 2]>::default().add([0.5, 0.5]);
        assert_eq!(tree.x0, 0.0);
        assert_eq!(tree.y0, 0.0);
        assert_eq!(tree.x1, 1.0);
        assert_eq!(tree.y1, 1.0);

        // TODO: test adding more nodes when we have a getter.
    }
}
