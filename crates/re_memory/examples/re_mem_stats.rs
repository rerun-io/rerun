use std::sync::Arc;

use re_memory::{GenNode, Global, Node, Struct, TextTree as _, TrackingAllocator};

#[global_allocator]
pub static GLOBAL_ALLOCATOR: TrackingAllocator<std::alloc::System> =
    TrackingAllocator::new(std::alloc::System);

struct MyData {
    name: String,
    bytes: Vec<u8>,
    shared: Arc<String>,
}

impl GenNode for MyData {
    fn node(&self, global: &mut Global) -> Node {
        let Self {
            name,
            bytes,
            shared,
        } = self;
        Node::Struct(Struct {
            type_name: "MyData",
            fields: vec![
                ("name", name.node(global)),
                ("bytes", bytes.node(global)),
                ("shared", shared.node(global)),
            ],
        })
    }
}

fn main() {
    let data = MyData {
        name: "Hello World!".to_owned(),
        bytes: vec![0; 1024],
        shared: Arc::new("Hello World!".to_owned()),
    };

    let mut global = Global::default();
    let node = data.node(&mut global);
    println!("{}\n", GLOBAL_ALLOCATOR.text_tree());
    println!("{}\n", global.text_tree());
    println!("{}\n\n", node.text_tree());
}
