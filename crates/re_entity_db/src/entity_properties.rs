#[cfg(feature = "serde")]
use re_log_types::EntityPath;

// ----------------------------------------------------------------------------

/// Properties for a collection of entities.
#[cfg(feature = "serde")]
#[derive(Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct EntityPropertyMap {
    props: nohash_hasher::IntMap<EntityPath, EntityProperties>,
}

#[cfg(feature = "serde")]
impl EntityPropertyMap {
    #[inline]
    pub fn get(&self, entity_path: &EntityPath) -> EntityProperties {
        self.props.get(entity_path).cloned().unwrap_or_default()
    }

    #[inline]
    pub fn get_opt(&self, entity_path: &EntityPath) -> Option<&EntityProperties> {
        self.props.get(entity_path)
    }

    /// Updates the properties for a given entity path.
    ///
    /// If an existing value is already in the map for the given entity path, the new value is merged
    /// with the existing value. When merging, auto values that were already set inside the map are
    /// preserved.
    #[inline]
    pub fn update(&mut self, entity_path: EntityPath, prop: EntityProperties) {
        if prop == EntityProperties::default() {
            self.props.remove(&entity_path); // save space
        } else if self.props.contains_key(&entity_path) {
            let merged = self
                .props
                .get(&entity_path)
                .cloned()
                .unwrap_or_default()
                .merge_with(&prop);
            self.props.insert(entity_path, merged);
        } else {
            self.props.insert(entity_path, prop);
        }
    }

    /// Overrides the properties for a given entity path.
    ///
    /// Like `update`, but auto properties are always updated.
    pub fn overwrite_properties(&mut self, entity_path: EntityPath, prop: EntityProperties) {
        if prop == EntityProperties::default() {
            self.props.remove(&entity_path); // save space
        } else {
            self.props.insert(entity_path, prop);
        }
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (&EntityPath, &EntityProperties)> {
        self.props.iter()
    }

    /// Determine whether this `EntityPropertyMap` has user-edits relative to another `EntityPropertyMap`
    pub fn has_edits(&self, other: &Self) -> bool {
        self.props.len() != other.props.len()
            || self.props.iter().any(|(key, val)| {
                other
                    .props
                    .get(key)
                    .map_or(true, |other_val| val.has_edits(other_val))
            })
    }
}

#[cfg(feature = "serde")]
impl FromIterator<(EntityPath, EntityProperties)> for EntityPropertyMap {
    fn from_iter<T: IntoIterator<Item = (EntityPath, EntityProperties)>>(iter: T) -> Self {
        Self {
            props: iter.into_iter().collect(),
        }
    }
}

// ----------------------------------------------------------------------------

#[cfg(feature = "serde")]
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct EntityProperties {
    // TODO(#5067): Test property used so we don't have to continuously adjust existing tests while we're dismantling `EntityProperties`.
    pub test_property: bool,
}

#[cfg(feature = "serde")]
impl Default for EntityProperties {
    fn default() -> Self {
        Self {
            test_property: true,
        }
    }
}

#[cfg(feature = "serde")]
impl EntityProperties {
    /// Multiply/and these together.
    pub fn with_child(&self, child: &Self) -> Self {
        Self {
            test_property: self.test_property && child.test_property,
        }
    }

    /// Merge this `EntityProperty` with the values from another `EntityProperty`.
    ///
    /// When merging, other values are preferred over self values unless they are auto
    /// values, in which case self values are preferred.
    ///
    /// This is important to combine the base-layer of up-to-date auto-values with values
    /// loaded from the Blueprint store where the Auto values are not up-to-date.
    #[allow(clippy::unused_self)] // TODO(andreas): we're on the way out anyways :)
    pub fn merge_with(&self, other: &Self) -> Self {
        Self {
            test_property: other.test_property,
        }
    }

    /// Determine whether this `EntityProperty` has user-edits relative to another `EntityProperty`
    pub fn has_edits(&self, other: &Self) -> bool {
        let Self { test_property } = self;

        test_property != &other.test_property
    }
}
