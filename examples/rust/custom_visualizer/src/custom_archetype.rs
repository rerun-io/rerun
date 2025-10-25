use rerun::{external::re_types::try_serialize_field, Component};

/// Custom archetype for drawing ??TODO?? in the 3D view.
#[derive(Default)]
pub struct Custom {
    pub positions: Option<rerun::SerializedComponentBatch>,
    pub colors: Option<rerun::SerializedComponentBatch>,
}

impl rerun::Archetype for Custom {
    fn name() -> rerun::ArchetypeName {
        "Custom".into()
    }

    fn display_name() -> &'static str {
        "Custom"
    }

    fn required_components() -> ::std::borrow::Cow<'static, [rerun::ComponentDescriptor]> {
        vec![Self::descriptor_positions()].into()
    }

    fn optional_components() -> std::borrow::Cow<'static, [rerun::ComponentDescriptor]> {
        vec![Self::descriptor_colors()].into()
    }
}

impl Custom {
    /// Returns the [`rerun::ComponentDescriptor`] for [`Self::positions`].
    #[inline]
    pub fn descriptor_positions() -> rerun::ComponentDescriptor {
        rerun::ComponentDescriptor {
            archetype: Some("Custom".into()),
            component: "Custom:positions".into(),
            component_type: Some(rerun::components::Position3D::name()),
        }
    }

    /// Returns the [`rerun::ComponentDescriptor`] for [`Self::colors`].
    #[inline]
    pub fn descriptor_colors() -> rerun::ComponentDescriptor {
        rerun::ComponentDescriptor {
            archetype: Some("Custom".into()),
            component: "Custom:colors".into(),
            component_type: Some(rerun::components::Color::name()),
        }
    }

    #[inline]
    pub fn new(
        positions: impl IntoIterator<Item = impl Into<rerun::components::Position3D>>,
    ) -> Self {
        Self::default().with_positions(positions)
    }

    #[inline]
    pub fn with_positions(
        mut self,
        positions: impl IntoIterator<Item = impl Into<rerun::components::Position3D>>,
    ) -> Self {
        self.positions = try_serialize_field(Self::descriptor_positions(), positions);
        self
    }

    #[inline]
    pub fn with_colors(
        mut self,
        vertex_colors: impl IntoIterator<Item = impl Into<rerun::components::Color>>,
    ) -> Self {
        self.colors = try_serialize_field(Self::descriptor_colors(), vertex_colors);
        self
    }
}

impl rerun::AsComponents for Custom {
    #[inline]
    fn as_serialized_batches(&self) -> Vec<rerun::SerializedComponentBatch> {
        [self.positions.clone(), self.colors.clone()]
            .into_iter()
            .flatten()
            .collect()
    }
}
