//! Demo application for the memory flamegraph widget.
//!
//! Run with:
//! ```sh
//! cargo run --example demo -p re_memory_view
//! ```

use re_byte_size::{MemUsageNode, MemUsageTree, NamedMemUsageTree};

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1200.0, 800.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Memory flamegraph demo",
        options,
        Box::new(|_cc| Ok(Box::new(DemoApp::new()))),
    )
}

struct DemoApp {
    tree: NamedMemUsageTree,
}

impl DemoApp {
    fn new() -> Self {
        Self {
            tree: NamedMemUsageTree::new("Demo App", create_demo_tree()),
        }
    }
}

impl eframe::App for DemoApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Memory flamegraph demo");
            ui.separator();

            ui.label("Controls:");
            ui.label("• Ctrl+Scroll to zoom");
            ui.label("• Scroll horizontally to pan");
            ui.label("• Double-click scope to zoom to it");
            ui.label("• Double-click background to reset view");
            ui.separator();

            re_memory_view::memory_flamegraph_ui(ui, &self.tree);
        });
    }
}

/// Create a demo memory tree for testing.
/// This simulates a realistic Rerun viewer session with various loaded data.
fn create_demo_tree() -> MemUsageTree {
    let mut root = MemUsageNode::new();

    // Entity Database - the main data store
    let mut entity_db = MemUsageNode::new();

    // Chunk store with multiple recordings
    let mut chunk_store = MemUsageNode::new();

    let mut recording1 = MemUsageNode::new();
    recording1.add("row_data", MemUsageTree::Bytes(85_000_000)); // 85 MB
    recording1.add("indices", MemUsageTree::Bytes(12_000_000)); // 12 MB
    recording1.add("metadata", MemUsageTree::Bytes(3_000_000)); // 3 MB
    chunk_store.add("robotics/outdoor_run_001", recording1.into_tree());

    let mut recording2 = MemUsageNode::new();
    recording2.add("row_data", MemUsageTree::Bytes(145_000_000)); // 145 MB
    recording2.add("indices", MemUsageTree::Bytes(18_000_000)); // 18 MB
    recording2.add("metadata", MemUsageTree::Bytes(4_500_000)); // 4.5 MB
    chunk_store.add("robotics/indoor_slam_002", recording2.into_tree());

    let mut recording3 = MemUsageNode::new();
    recording3.add("row_data", MemUsageTree::Bytes(62_000_000)); // 62 MB
    recording3.add("indices", MemUsageTree::Bytes(9_000_000)); // 9 MB
    recording3.add("metadata", MemUsageTree::Bytes(2_000_000)); // 2 MB
    chunk_store.add("cv/traffic_analysis", recording3.into_tree());

    entity_db.add("chunk_store", chunk_store.into_tree());

    // Time histograms for timeline scrubbing
    let mut time_histograms = MemUsageNode::new();
    time_histograms.add("log_time", MemUsageTree::Bytes(1_200_000)); // 1.2 MB
    time_histograms.add("log_tick", MemUsageTree::Bytes(800_000)); // 800 KB
    time_histograms.add("frame_nr", MemUsageTree::Bytes(600_000)); // 600 KB
    entity_db.add("time_histograms", time_histograms.into_tree());

    entity_db.add("entity_path_tree", MemUsageTree::Bytes(4_500_000)); // 4.5 MB
    entity_db.add("data_source", MemUsageTree::Bytes(800_000)); // 800 KB

    root.add("entity_db", entity_db.into_tree());

    // Viewer caches - where processed data lives
    let mut caches = MemUsageNode::new();

    // Image cache with decoded images
    let mut image_cache = MemUsageNode::new();
    image_cache.add(
        "rgb_camera/front/000124.jpg",
        MemUsageTree::Bytes(18_000_000),
    ); // 18 MB
    image_cache.add(
        "rgb_camera/front/000125.jpg",
        MemUsageTree::Bytes(18_500_000),
    ); // 18.5 MB
    image_cache.add(
        "rgb_camera/front/000126.jpg",
        MemUsageTree::Bytes(17_800_000),
    ); // 17.8 MB
    image_cache.add(
        "rgb_camera/left/000124.jpg",
        MemUsageTree::Bytes(16_000_000),
    ); // 16 MB
    image_cache.add(
        "rgb_camera/left/000125.jpg",
        MemUsageTree::Bytes(16_200_000),
    ); // 16.2 MB
    image_cache.add(
        "depth_camera/front/000124.png",
        MemUsageTree::Bytes(8_000_000),
    ); // 8 MB
    image_cache.add(
        "depth_camera/front/000125.png",
        MemUsageTree::Bytes(8_100_000),
    ); // 8.1 MB
    caches.add("image_decode_cache", image_cache.into_tree());

    // Video frame cache
    let mut video_cache = MemUsageNode::new();
    video_cache.add("video/dashboard_cam.mp4", MemUsageTree::Bytes(95_000_000)); // 95 MB
    video_cache.add("video/rear_view.mp4", MemUsageTree::Bytes(67_000_000)); // 67 MB
    caches.add("video_cache", video_cache.into_tree());

    // Mesh cache with processed 3D data
    let mut mesh_cache = MemUsageNode::new();
    mesh_cache.add("lidar/scan_mesh_001", MemUsageTree::Bytes(32_000_000)); // 32 MB
    mesh_cache.add("lidar/scan_mesh_002", MemUsageTree::Bytes(35_000_000)); // 35 MB
    mesh_cache.add("reconstruction/map_mesh", MemUsageTree::Bytes(78_000_000)); // 78 MB
    caches.add("mesh_cache", mesh_cache.into_tree());

    // Tensor cache
    let mut tensor_cache = MemUsageNode::new();
    tensor_cache.add("neural_net/feature_maps", MemUsageTree::Bytes(28_000_000)); // 28 MB
    tensor_cache.add("neural_net/embeddings", MemUsageTree::Bytes(15_000_000)); // 15 MB
    tensor_cache.add("heatmaps/detections", MemUsageTree::Bytes(12_000_000)); // 12 MB
    caches.add("tensor_cache", tensor_cache.into_tree());

    caches.add("point_cloud_cache", MemUsageTree::Bytes(52_000_000)); // 52 MB
    caches.add("text_layout_cache", MemUsageTree::Bytes(3_500_000)); // 3.5 MB

    root.add("caches", caches.into_tree());

    // Renderer - GPU resources and render state
    let mut renderer = MemUsageNode::new();

    let mut gpu_resources = MemUsageNode::new();
    gpu_resources.add("vertex_buffers", MemUsageTree::Bytes(42_000_000)); // 42 MB
    gpu_resources.add("index_buffers", MemUsageTree::Bytes(18_000_000)); // 18 MB
    gpu_resources.add("uniform_buffers", MemUsageTree::Bytes(5_000_000)); // 5 MB
    gpu_resources.add("staging_buffers", MemUsageTree::Bytes(24_000_000)); // 24 MB
    renderer.add("gpu_resources", gpu_resources.into_tree());

    let mut textures = MemUsageNode::new();
    textures.add("color_attachments", MemUsageTree::Bytes(36_000_000)); // 36 MB
    textures.add("depth_attachments", MemUsageTree::Bytes(20_000_000)); // 20 MB
    textures.add("texture_atlas", MemUsageTree::Bytes(28_000_000)); // 28 MB
    textures.add("shadow_maps", MemUsageTree::Bytes(16_000_000)); // 16 MB
    renderer.add("textures", textures.into_tree());

    renderer.add("shader_modules", MemUsageTree::Bytes(8_500_000)); // 8.5 MB
    renderer.add("render_pipelines", MemUsageTree::Bytes(2_000_000)); // 2 MB
    renderer.add("bind_groups", MemUsageTree::Bytes(1_500_000)); // 1.5 MB

    root.add("renderer", renderer.into_tree());

    // UI State
    let mut ui_state = MemUsageNode::new();
    ui_state.add("egui_context", MemUsageTree::Bytes(8_000_000)); // 8 MB
    ui_state.add("viewport_layout", MemUsageTree::Bytes(500_000)); // 500 KB
    ui_state.add("selection_state", MemUsageTree::Bytes(1_200_000)); // 1.2 MB
    ui_state.add("blueprint_tree", MemUsageTree::Bytes(3_500_000)); // 3.5 MB

    let mut panels = MemUsageNode::new();
    panels.add("time_panel", MemUsageTree::Bytes(2_000_000)); // 2 MB
    panels.add("selection_panel", MemUsageTree::Bytes(1_500_000)); // 1.5 MB
    panels.add("blueprint_panel", MemUsageTree::Bytes(1_800_000)); // 1.8 MB
    ui_state.add("panels", panels.into_tree());

    root.add("ui_state", ui_state.into_tree());

    // Network & IO
    let mut network = MemUsageNode::new();
    network.add("tcp_receive_buffer", MemUsageTree::Bytes(12_000_000)); // 12 MB
    network.add("tcp_send_buffer", MemUsageTree::Bytes(4_000_000)); // 4 MB
    network.add("websocket_buffers", MemUsageTree::Bytes(6_000_000)); // 6 MB
    network.add("file_mmap", MemUsageTree::Bytes(25_000_000)); // 25 MB
    root.add("network_io", network.into_tree());

    // Analytics & Telemetry
    let mut analytics = MemUsageNode::new();
    analytics.add("event_buffer", MemUsageTree::Bytes(1_500_000)); // 1.5 MB
    analytics.add("metrics_history", MemUsageTree::Bytes(2_800_000)); // 2.8 MB
    root.add("analytics", analytics.into_tree());

    // Small allocations
    root.add("log_messages", MemUsageTree::Bytes(800_000)); // 800 KB
    root.add("command_history", MemUsageTree::Bytes(400_000)); // 400 KB
    root.add("app_config", MemUsageTree::Bytes(150_000)); // 150 KB

    root.into_tree()
}
