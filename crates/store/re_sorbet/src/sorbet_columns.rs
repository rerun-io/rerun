use arrow::datatypes::{Field as ArrowField, Fields as ArrowFields};

use re_log_types::EntityPath;

use crate::{
    ColumnKind, ComponentColumnDescriptor, IndexColumnDescriptor, InvalidSorbetSchema,
    RowIdColumnDescriptor,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SorbetColumnDescriptors {
    /// The primary row id column.
    /// If present, it is always the first column.
    pub row_id: Option<RowIdColumnDescriptor>,

    /// Index columns (timelines).
    pub indices: Vec<IndexColumnDescriptor>,

    /// The actual component data
    pub components: Vec<ComponentColumnDescriptor>,
}

impl SorbetColumnDescriptors {
    /// Total number of columns in this chunk,
    /// including the row id column, the index columns,
    /// and the data columns.
    pub fn num_columns(&self) -> usize {
        let Self {
            row_id,
            indices,
            components,
        } = self;
        row_id.is_some() as usize + indices.len() + components.len()
    }

    pub fn arrow_fields(&self) -> Vec<ArrowField> {
        let Self {
            row_id,
            indices,
            components,
        } = self;
        let mut fields: Vec<ArrowField> = Vec::with_capacity(self.num_columns());
        if let Some(row_id) = row_id {
            fields.push(row_id.to_arrow_field());
        }
        fields.extend(indices.iter().map(|column| column.to_arrow_field()));
        fields.extend(
            components
                .iter()
                .map(|column| column.to_arrow_field(crate::BatchType::Chunk)),
        );
        fields
    }
}

impl SorbetColumnDescriptors {
    fn try_from_arrow_fields(
        chunk_entity_path: Option<&EntityPath>,
        fields: &ArrowFields,
    ) -> Result<Self, InvalidSorbetSchema> {
        let mut row_ids = Vec::new();
        let mut indices = Vec::new();
        let mut components = Vec::new();

        for field in fields {
            let field = field.as_ref();
            let column_kind = ColumnKind::try_from(field)?;
            match column_kind {
                ColumnKind::RowId => {
                    if indices.is_empty() && components.is_empty() {
                        row_ids.push(RowIdColumnDescriptor::try_from(field)?);
                    } else {
                        return Err(InvalidSorbetSchema::custom(
                            "RowId column must be the first column",
                        ));
                    }
                }

                ColumnKind::Index => {
                    if components.is_empty() {
                        indices.push(IndexColumnDescriptor::try_from(field)?);
                    } else {
                        return Err(InvalidSorbetSchema::custom(
                            "Index columns must come before any data columns",
                        ));
                    }
                }

                ColumnKind::Component => {
                    components.push(ComponentColumnDescriptor::from_arrow_field(
                        chunk_entity_path,
                        field,
                    ));
                }
            }
        }

        if row_ids.len() > 1 {
            return Err(InvalidSorbetSchema::custom(
                "Multiple row_id columns are not supported",
            ));
        }

        Ok(Self {
            row_id: row_ids.pop(),
            indices,
            components,
        })
    }
}
