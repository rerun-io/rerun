//! Memory usage tree for profiling and debugging.

/// A snapshot of memory usage of a value,
/// produced by [`MemUsageTreeCapture::capture_mem_usage_tree`].
pub enum MemUsageTree {
    /// A leaf node with a known size in bytes.
    Bytes(u64),

    /// A node with children (e.g. a struct or collection).
    Node(MemUsageNode),
}

impl Default for MemUsageTree {
    fn default() -> Self {
        Self::Bytes(0)
    }
}

impl From<u64> for MemUsageTree {
    fn from(size: u64) -> Self {
        Self::Bytes(size)
    }
}

impl MemUsageTree {
    /// The size (in bytes) of this tree.
    pub fn size_bytes(&self) -> u64 {
        match self {
            Self::Bytes(size) => *size,
            Self::Node(node) => node.size_bytes,
        }
    }
}

/// A named child in a [`MemUsageNode`].
pub struct NamedMemUsageTree {
    /// Name of this child node.
    pub name: String,

    /// The memory usage tree of this child.
    pub value: MemUsageTree,
}

impl NamedMemUsageTree {
    pub fn new(name: impl Into<String>, child: impl Into<MemUsageTree>) -> Self {
        Self {
            name: name.into(),
            value: child.into(),
        }
    }

    pub fn size_bytes(&self) -> u64 {
        self.value.size_bytes()
    }
}

/// A node in a [`MemUsageTree`] with children.
#[derive(Default)]
pub struct MemUsageNode {
    /// Children of this node.
    children: Vec<NamedMemUsageTree>,

    /// Size of us and all children in bytes.
    size_bytes: u64,
}

impl MemUsageNode {
    /// Create a new empty node.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a child to this node.
    ///
    /// The child's size will be added to this node's total size.
    pub fn add(&mut self, name: impl Into<String>, child: impl Into<MemUsageTree>) {
        self.add_named_child(NamedMemUsageTree::new(name, child));
    }

    pub fn add_named_child(&mut self, tree: NamedMemUsageTree) {
        self.size_bytes += tree.size_bytes();
        self.children.push(tree);
    }

    pub fn with_named_child(mut self, tree: NamedMemUsageTree) -> Self {
        self.add_named_child(tree);
        self
    }

    pub fn with_child(mut self, name: impl Into<String>, child: impl Into<MemUsageTree>) -> Self {
        self.add_named_child(NamedMemUsageTree::new(name, child));
        self
    }

    /// Get the total size in bytes of this node and all its children.
    pub fn size_bytes(&self) -> u64 {
        self.size_bytes
    }

    /// Get the children of this node.
    pub fn children(&self) -> &[NamedMemUsageTree] {
        &self.children
    }

    /// Set the total size in bytes of this node.
    ///
    /// Usually the size is computed automatically when adding children,
    /// but this can be used to override it if needed,
    /// for instance if we have a more accurate size measurement produced some other way.
    pub fn with_total_size_bytes(mut self, size_bytes: u64) -> MemUsageTree {
        self.size_bytes = size_bytes;
        self.into_tree()
    }

    /// Convert this node into a [`MemUsageTree::Node`].
    pub fn into_tree(self) -> MemUsageTree {
        MemUsageTree::Node(self)
    }
}

/// A trait for capturing a detailed [`MemUsageTree`] of a value.
///
/// Implement this for high-level types to get detailed memory usage breakdowns.
/// Lower-level types should implement [`SizeBytes`](crate::SizeBytes) instead.
pub trait MemUsageTreeCapture {
    fn capture_mem_usage_tree(&self) -> MemUsageTree;
}

impl<T: MemUsageTreeCapture> MemUsageTreeCapture for Option<T> {
    fn capture_mem_usage_tree(&self) -> MemUsageTree {
        match self {
            Some(value) => value.capture_mem_usage_tree(),
            None => MemUsageTree::Bytes(0),
        }
    }
}

impl<K, V, S> MemUsageTreeCapture for std::collections::HashMap<K, V, S>
where
    K: std::fmt::Display,
    V: MemUsageTreeCapture,
{
    fn capture_mem_usage_tree(&self) -> MemUsageTree {
        let mut node = MemUsageNode::new();

        for (key, value) in self {
            // Assumes the keys are small enough
            node.add(key.to_string(), value.capture_mem_usage_tree());
        }

        node.into_tree()
    }
}
