//! View types for blueprint configuration.

use uuid::Uuid;

use re_log_types::EntityPath;
use re_sdk_types::blueprint::archetypes::{ViewBlueprint, ViewContents};
use re_sdk_types::blueprint::components::{QueryExpression, ViewClass};
use re_sdk_types::components::{Name, Visible};
use re_sdk_types::datatypes::Bool;

/// Internal view type. Use specific view types like [`TimeSeriesView`], [`MapView`], etc.
#[derive(Debug)]
pub struct View {
    pub(crate) id: Uuid,
    pub(crate) class_identifier: String,
    pub(crate) name: Option<String>,
    pub(crate) origin: EntityPath,
    pub(crate) contents: Vec<String>, // Query expressions
    pub(crate) visible: Option<bool>,
}

impl View {
    /// Get the blueprint path for this view.
    pub fn blueprint_path(&self) -> EntityPath {
        format!("view/{}", self.id).into()
    }

    pub(crate) fn log_to_stream(
        &self,
        stream: &crate::RecordingStream,
    ) -> crate::RecordingStreamResult<()> {
        let view_contents = ViewContents::new(
            self.contents
                .iter()
                .map(|q| QueryExpression(q.clone().into())),
        );

        stream.log(
            format!("{}/ViewContents", self.blueprint_path()),
            &view_contents,
        )?;

        let mut arch = ViewBlueprint::new(ViewClass(self.class_identifier.clone().into()));

        if let Some(ref name) = self.name {
            arch = arch.with_display_name(Name(name.clone().into()));
        }

        arch = arch.with_space_origin(self.origin.to_string());

        if let Some(visible) = self.visible {
            arch = arch.with_visible(Visible(Bool(visible)));
        }

        stream.log(self.blueprint_path(), &arch)
    }
}

/// Time series view for scalars over time.
pub struct TimeSeriesView(pub(crate) View);

impl TimeSeriesView {
    /// Create a new time series view.
    pub fn new(name: impl Into<String>) -> Self {
        Self(View {
            id: Uuid::new_v4(),
            class_identifier: "TimeSeries".into(),
            name: Some(name.into()),
            origin: "/".into(),
            contents: vec!["/**".into()], // Default: show everything
            visible: None,
        })
    }

    /// Set the origin entity path.
    pub fn with_origin(mut self, origin: impl Into<EntityPath>) -> Self {
        self.0.origin = origin.into();
        self
    }

    /// Set the contents query expressions.
    pub fn with_contents(mut self, queries: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.0.contents = queries.into_iter().map(Into::into).collect();
        self
    }

    /// Set visibility.
    pub fn with_visible(mut self, visible: bool) -> Self {
        self.0.visible = Some(visible);
        self
    }
}

/// Spatial 2D view.
pub struct Spatial2DView(pub(crate) View);

impl Spatial2DView {
    /// Create a new spatial 2D view.
    pub fn new(name: impl Into<String>) -> Self {
        Self(View {
            id: Uuid::new_v4(),
            class_identifier: "2D".into(),
            name: Some(name.into()),
            origin: "/".into(),
            contents: vec!["/**".into()], // Default: show everything
            visible: None,
        })
    }

    /// Set the origin entity path.
    pub fn with_origin(mut self, origin: impl Into<EntityPath>) -> Self {
        self.0.origin = origin.into();
        self
    }

    /// Set the contents query expressions.
    pub fn with_contents(mut self, queries: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.0.contents = queries.into_iter().map(Into::into).collect();
        self
    }

    /// Set visibility.
    pub fn with_visible(mut self, visible: bool) -> Self {
        self.0.visible = Some(visible);
        self
    }
}

/// Spatial 3D view.
pub struct Spatial3DView(pub(crate) View);

impl Spatial3DView {
    /// Create a new spatial 3D view.
    pub fn new(name: impl Into<String>) -> Self {
        Self(View {
            id: Uuid::new_v4(),
            class_identifier: "3D".into(),
            name: Some(name.into()),
            origin: "/".into(),
            contents: vec!["/**".into()], // Default: show everything
            visible: None,
        })
    }

    /// Set the origin entity path.
    pub fn with_origin(mut self, origin: impl Into<EntityPath>) -> Self {
        self.0.origin = origin.into();
        self
    }

    /// Set the contents query expressions.
    pub fn with_contents(mut self, queries: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.0.contents = queries.into_iter().map(Into::into).collect();
        self
    }

    /// Set visibility.
    pub fn with_visible(mut self, visible: bool) -> Self {
        self.0.visible = Some(visible);
        self
    }
}

/// Map view for geospatial data.
pub struct MapView(pub(crate) View);

impl MapView {
    /// Create a new map view.
    pub fn new(name: impl Into<String>) -> Self {
        Self(View {
            id: Uuid::new_v4(),
            class_identifier: "Map".into(),
            name: Some(name.into()),
            origin: "/".into(),
            contents: vec!["/**".into()],
            visible: None,
        })
    }

    /// Set the origin entity path.
    pub fn with_origin(mut self, origin: impl Into<EntityPath>) -> Self {
        self.0.origin = origin.into();
        self
    }

    /// Set the contents query expressions.
    pub fn with_contents(mut self, queries: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.0.contents = queries.into_iter().map(Into::into).collect();
        self
    }

    /// Set visibility.
    pub fn with_visible(mut self, visible: bool) -> Self {
        self.0.visible = Some(visible);
        self
    }
}

/// Text document view for markdown rendering.
pub struct TextDocumentView(pub(crate) View);

impl TextDocumentView {
    /// Create a new text document view.
    pub fn new(name: impl Into<String>) -> Self {
        Self(View {
            id: Uuid::new_v4(),
            class_identifier: "TextDocument".into(),
            name: Some(name.into()),
            origin: "/".into(),
            contents: vec!["/**".into()],
            visible: None,
        })
    }

    /// Set the origin entity path.
    pub fn with_origin(mut self, origin: impl Into<EntityPath>) -> Self {
        self.0.origin = origin.into();
        self
    }

    /// Set the contents query expressions.
    pub fn with_contents(mut self, queries: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.0.contents = queries.into_iter().map(Into::into).collect();
        self
    }

    /// Set visibility.
    pub fn with_visible(mut self, visible: bool) -> Self {
        self.0.visible = Some(visible);
        self
    }
}
