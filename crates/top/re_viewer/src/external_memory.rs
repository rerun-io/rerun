//! External users of memory, i.e things that allocate memory but
//! aren't accessible in this crate. Which is used for GC and
//! memory usage information.

use re_byte_size::NamedMemUsageTree;

/// Something contributing to application memory that is not visible in this
/// crate.
pub trait ExternalMemoryUser {
    /// Will return `Some` while the memory user exists. After which
    /// it will always return `None`.
    fn capture(&mut self) -> Option<NamedMemUsageTree>;
}

#[derive(Default)]
pub struct ExternalMemoryUsers {
    users: Vec<Box<dyn ExternalMemoryUser>>,

    latest_capture: Vec<NamedMemUsageTree>,
    total_external_memory: u64,
}

impl ExternalMemoryUsers {
    pub fn default_users() -> Self {
        let mut this = Self::default();

        struct AllocatorTrackingOverhead;

        impl ExternalMemoryUser for AllocatorTrackingOverhead {
            fn capture(&mut self) -> Option<NamedMemUsageTree> {
                re_memory::accounting_allocator::tracking_stats().map(|tracking_stats| {
                    NamedMemUsageTree::new(
                        "Allocator tracking",
                        tracking_stats.overhead.size as u64,
                    )
                })
            }
        }

        this.add(Box::new(AllocatorTrackingOverhead));

        this
    }

    pub fn captured_trees(&self) -> &[NamedMemUsageTree] {
        &self.latest_capture
    }

    pub fn total_external_memory(&self) -> u64 {
        self.total_external_memory
    }

    /// Capture memory usage trees for all registered external memory users.
    pub fn update(&mut self) {
        re_tracing::profile_function!();

        self.latest_capture.clear();

        self.users.retain_mut(|user| {
            if let Some(tree) = user.capture() {
                self.latest_capture.push(tree);

                true
            } else {
                false
            }
        });

        self.total_external_memory = self.latest_capture.iter().map(|t| t.size_bytes()).sum();
    }

    pub fn add(&mut self, user: Box<dyn ExternalMemoryUser>) {
        self.users.push(user);
    }
}
