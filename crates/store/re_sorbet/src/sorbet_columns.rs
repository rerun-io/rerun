use arrow::datatypes::{Field as ArrowField, Fields as ArrowFields};
use nohash_hasher::IntSet;

use re_log_types::{EntityPath, TimelineName};

use crate::{
    ColumnDescriptor, ColumnKind, ComponentColumnDescriptor, ComponentColumnSelector,
    IndexColumnDescriptor, RowIdColumnDescriptor, SorbetError, TimeColumnSelector,
};

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
#[expect(clippy::enum_variant_names)]
pub enum ColumnSelectorResolveError {
    #[error("Column for component '{0}' not found")]
    ComponentNotFound(String),

    #[error(
        "Multiple columns were found for component '{0}'. Consider using a more specific selector."
    )]
    MultipleComponentColumnFound(String),

    #[error("Index column for timeline '{0}' not found")]
    TimelineNotFound(TimelineName),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SorbetColumnDescriptors {
    pub columns: Vec<ColumnDescriptor>,
}

impl std::ops::Deref for SorbetColumnDescriptors {
    type Target = [ColumnDescriptor];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.columns
    }
}

impl IntoIterator for SorbetColumnDescriptors {
    type Item = ColumnDescriptor;
    type IntoIter = std::vec::IntoIter<ColumnDescriptor>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.columns.into_iter()
    }
}

impl SorbetColumnDescriptors {
    /// Debug-only sanity check.
    #[inline]
    #[track_caller]
    pub fn sanity_check(&self) {
        for column in &self.columns {
            column.sanity_check();
        }
    }

    /// Total number of columns in this chunk,
    /// including the row id column, the index columns,
    /// and the data columns.
    pub fn num_columns(&self) -> usize {
        self.columns.len()
    }

    /// All unique entity paths present in the view contents.
    pub fn entity_paths(&self) -> IntSet<EntityPath> {
        self.columns
            .iter()
            .filter_map(|col| col.entity_path().cloned())
            .collect()
    }

    pub fn row_id_columns(&self) -> impl Iterator<Item = &RowIdColumnDescriptor> {
        self.columns.iter().filter_map(|descr| {
            if let ColumnDescriptor::RowId(descr) = descr {
                Some(descr)
            } else {
                None
            }
        })
    }

    pub fn index_columns(&self) -> impl Iterator<Item = &IndexColumnDescriptor> {
        self.columns.iter().filter_map(|descr| {
            if let ColumnDescriptor::Time(descr) = descr {
                Some(descr)
            } else {
                None
            }
        })
    }

    pub fn component_columns(&self) -> impl Iterator<Item = &ComponentColumnDescriptor> {
        self.columns.iter().filter_map(|descr| {
            if let ColumnDescriptor::Component(descr) = descr {
                Some(descr)
            } else {
                None
            }
        })
    }

    /// Resolve the provided index column selector. Returns `None` if no corresponding column was
    /// found.
    pub fn resolve_index_column_selector(
        &self,
        index_column_selector: &TimeColumnSelector,
    ) -> Result<&IndexColumnDescriptor, ColumnSelectorResolveError> {
        self.index_columns()
            .find(|column| column.timeline_name() == index_column_selector.timeline)
            .ok_or(ColumnSelectorResolveError::TimelineNotFound(
                index_column_selector.timeline,
            ))
    }

    /// Resolve the provided component column selector. Returns `None` if no corresponding column
    /// was found.
    ///
    /// Matching strategy:
    /// - The entity path must match exactly.
    /// - The first component with a fully matching name is returned (there shouldn't be more than
    ///   one for now).
    /// - If no exact match is found, a partial match is attempted using
    ///   [`re_types_core::ComponentName::matches`] and is returned only if it is unique.
    // TODO(#6889): this will need to be fully revisited with tagged components
    // TODO(ab): this is related but different from `re_chunk_store::resolve_component_selector()`.
    // It is likely that only one of these should eventually remain.
    pub fn resolve_component_column_selector(
        &self,
        component_column_selector: &ComponentColumnSelector,
    ) -> Result<&ComponentColumnDescriptor, ColumnSelectorResolveError> {
        let ComponentColumnSelector {
            entity_path,
            component_name,
        } = component_column_selector;

        // happy path: exact component name match
        let exact_match = self.component_columns().find(|column| {
            column.component_name.as_str() == component_name && &column.entity_path == entity_path
        });

        if let Some(exact_match) = exact_match {
            return Ok(exact_match);
        }

        // fallback: use `ComponentName::match` and check that we have a single result
        let mut partial_match = self.component_columns().filter(|column| {
            column.component_name.matches(component_name) && &column.entity_path == entity_path
        });

        let first_match = partial_match.next();

        // Note: non-unique partial match is highly unlikely for now, but will become more likely
        // with tagged components.
        if partial_match.next().is_none() {
            first_match.ok_or(ColumnSelectorResolveError::ComponentNotFound(
                component_name.clone(),
            ))
        } else {
            Err(ColumnSelectorResolveError::MultipleComponentColumnFound(
                component_name.clone(),
            ))
        }
    }

    pub fn arrow_fields(&self, batch_type: crate::BatchType) -> Vec<ArrowField> {
        self.columns
            .iter()
            .map(|c| c.to_arrow_field(batch_type))
            .collect()
    }
}

impl SorbetColumnDescriptors {
    pub fn try_from_arrow_fields(
        chunk_entity_path: Option<&EntityPath>,
        fields: &ArrowFields,
    ) -> Result<Self, SorbetError> {
        let mut columns = Vec::with_capacity(fields.len());

        for field in fields {
            let field = field.as_ref();
            let column_kind = ColumnKind::try_from(field)?;

            let descr = match column_kind {
                ColumnKind::RowId => {
                    ColumnDescriptor::RowId(RowIdColumnDescriptor::try_from(field)?)
                }

                ColumnKind::Index => {
                    ColumnDescriptor::Time(IndexColumnDescriptor::try_from(field)?)
                }

                ColumnKind::Component => ColumnDescriptor::Component(
                    ComponentColumnDescriptor::from_arrow_field(chunk_entity_path, field),
                ),
            };

            columns.push(descr);
        }

        Ok(Self { columns })
    }
}
