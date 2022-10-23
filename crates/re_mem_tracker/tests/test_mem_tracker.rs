#[global_allocator]
static GLOBAL: re_mem_tracker::TrackingAllocator<std::alloc::System> =
    re_mem_tracker::TrackingAllocator::new(std::alloc::System);

#[test]
fn test_mem_tracker() {
    // pre-allocate stats tree so they don't count:
    {
        re_mem_tracker::track_allocs!("outer");
        re_mem_tracker::track_allocs!("inner");
    }

    let mut outer_vec = vec![];
    let mut inner_vec = vec![];
    {
        re_mem_tracker::track_allocs!("outer");
        outer_vec.resize(1024, 0_u8);
        {
            re_mem_tracker::track_allocs!("inner");
            inner_vec.resize(256, 0_u8);
        }
    }

    let mut tree = re_mem_tracker::thread_local_tree();
    let outer = tree.child("outer");

    assert_eq!(outer.stats.total_allocs(), 2);
    assert_eq!(outer.stats.total_bytes(), 1024 + 256);
    assert_eq!(outer.stats.unaccounted_allocs(), 1);
    assert_eq!(outer.stats.unaccounted_bytes(), 1024);

    let inner = outer.child("outer");
    assert_eq!(inner.stats.total_allocs(), 1);
    assert_eq!(inner.stats.total_bytes(), 256);
    assert_eq!(inner.stats.unaccounted_allocs(), 1);
    assert_eq!(inner.stats.unaccounted_bytes(), 256);
}
