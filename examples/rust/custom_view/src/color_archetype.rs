use rerun::Component as _;

/// Custom archetype that consists only of a color.
#[derive(Default)]
pub struct ColorArchetype {
    #[allow(dead_code)]
    colors: Option<rerun::SerializedComponentBatch>,
}

impl rerun::Archetype for ColorArchetype {
    type Indicator = rerun::GenericIndicatorComponent<Self>;

    fn indicator() -> rerun::SerializedComponentBatch {
        use rerun::ComponentBatch as _;
        #[allow(clippy::unwrap_used)]
        Self::Indicator::default().serialized().unwrap()
    }

    fn name() -> rerun::ArchetypeName {
        "InstanceColor".into()
    }

    fn display_name() -> &'static str {
        "Instance Color"
    }

    fn required_components() -> ::std::borrow::Cow<'static, [rerun::ComponentDescriptor]> {
        vec![Self::descriptor_colors()].into()
    }
}

impl ColorArchetype {
    #[inline]
    pub fn descriptor_colors() -> rerun::ComponentDescriptor {
        rerun::ComponentDescriptor {
            archetype_name: Some("InstanceColor".into()),
            component_name: rerun::Color::name(),
            archetype_field_name: Some("colors".into()),
        }
    }
}
