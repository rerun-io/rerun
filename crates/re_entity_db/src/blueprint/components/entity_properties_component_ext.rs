use crate::EntityProperties;

use super::EntityPropertiesComponent;

impl re_types_core::SizeBytes for EntityPropertiesComponent {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        // TODO(cmc): Implementing SizeBytes for this type would require a lot of effort,
        // which would be wasted since this is supposed to go away very soon.
        0
    }
}

impl Default for EntityPropertiesComponent {
    #[inline]
    fn default() -> Self {
        Self(EntityProperties::default())
    }
}
