// This file is autogenerated via build.rs.
// DO NOT EDIT.

static ONCE: ::std::sync::atomic::AtomicBool = ::std::sync::atomic::AtomicBool::new(false);

pub fn init() {
    if ONCE.swap(true, ::std::sync::atomic::Ordering::Relaxed) {
        return;
    }

    use crate::file_system::FileSystem as _;
    let fs = crate::MemFileSystem::get();

    {
        let virtpath = ::std::path::Path::new("crates/re_renderer/shader/generic_skybox.wgsl");
        fs.create_file(
            &virtpath,
            include_str!("../shader/generic_skybox.wgsl").into(),
        )
        .unwrap();
    }

    {
        let virtpath = ::std::path::Path::new("crates/re_renderer/shader/test_triangle.wgsl");
        fs.create_file(
            &virtpath,
            include_str!("../shader/test_triangle.wgsl").into(),
        )
        .unwrap();
    }

    {
        let virtpath = ::std::path::Path::new("crates/re_renderer/shader/screen_triangle.wgsl");
        fs.create_file(
            &virtpath,
            include_str!("../shader/screen_triangle.wgsl").into(),
        )
        .unwrap();
    }

    {
        let virtpath = ::std::path::Path::new("crates/re_renderer/shader/point_cloud.wgsl");
        fs.create_file(&virtpath, include_str!("../shader/point_cloud.wgsl").into())
            .unwrap();
    }

    {
        let virtpath = ::std::path::Path::new("crates/re_renderer/shader/frame_uniform.wgsl");
        fs.create_file(
            &virtpath,
            include_str!("../shader/frame_uniform.wgsl").into(),
        )
        .unwrap();
    }

    {
        let virtpath = ::std::path::Path::new("crates/re_renderer/shader/tonemap.wgsl");
        fs.create_file(&virtpath, include_str!("../shader/tonemap.wgsl").into())
            .unwrap();
    }

    {
        let virtpath = ::std::path::Path::new("crates/re_renderer/shader/lines.wgsl");
        fs.create_file(&virtpath, include_str!("../shader/lines.wgsl").into())
            .unwrap();
    }
}
