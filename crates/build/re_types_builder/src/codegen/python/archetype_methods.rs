//! Archetype-specific method generation for Python codegen.

use itertools::Itertools as _;
use unindent::unindent;

use super::classmethod_decorators;
use super::docs::quote_doc_lines;
use super::init_method::{compute_init_parameter_docs, quote_init_parameter_from_field};
use crate::{Object, Objects, Reporter};

pub fn quote_clear_methods(obj: &Object) -> String {
    let param_nones = obj
        .fields
        .iter()
        .map(|field| format!("{} = None,", field.name))
        .join("\n                ");

    let classname = &obj.name;

    unindent(&format!(
        r#"
        def __attrs_clear__(self) -> None:
            """Convenience method for calling `__attrs_init__` with all `None`s."""
            self.__attrs_init__(
                {param_nones}
            )

        @classmethod
        {extra_decorators}
        def _clear(cls) -> {classname}:
            """Produce an empty {classname}, bypassing `__init__`."""
            inst = cls.__new__(cls)
            inst.__attrs_clear__()
            return inst
        "#,
        extra_decorators = classmethod_decorators(obj)
    ))
}

pub fn quote_kwargs(obj: &Object) -> String {
    obj.fields
        .iter()
        .map(|field| {
            let field_name = field.snake_case_name();
            format!("'{field_name}': {field_name}")
        })
        .collect_vec()
        .join(",\n")
}

pub fn quote_component_field_mapping(obj: &Object) -> String {
    obj.fields
        .iter()
        .map(|field| {
            let field_name = field.snake_case_name();
            format!("'{}:{field_name}': {field_name}", obj.name)
        })
        .collect_vec()
        .join(",\n")
}

pub fn quote_partial_update_methods(reporter: &Reporter, obj: &Object, objects: &Objects) -> String {
    let name = &obj.name;

    let parameters = obj
        .fields
        .iter()
        .map(|field| {
            let mut field = field.clone();
            field.is_nullable = true;
            quote_init_parameter_from_field(&field, objects, &obj.fqname)
        })
        .collect_vec()
        .join(",\n");
    let parameters = indent::indent_by(8, parameters);

    let kwargs = quote_kwargs(obj);
    let kwargs = indent::indent_by(12, kwargs);

    let parameter_docs = compute_init_parameter_docs(reporter, obj, objects);
    let mut doc_string_lines = vec![format!("Update only some specific fields of a `{name}`.")];
    if !parameter_docs.is_empty() {
        doc_string_lines.push("\n".to_owned());
        doc_string_lines.push("Parameters".to_owned());
        doc_string_lines.push("----------".to_owned());
        doc_string_lines.push("clear_unset:".to_owned());
        doc_string_lines
            .push("    If true, all unspecified fields will be explicitly cleared.".to_owned());
        for doc in parameter_docs {
            doc_string_lines.push(doc);
        }
    }
    let doc_block = indent::indent_by(12, quote_doc_lines(doc_string_lines));

    unindent(&format!(
        r#"
        @classmethod
        {extra_decorators}
        def from_fields(
            cls,
            *,
            clear_unset: bool = False,
            {parameters},
        ) -> {name}:
            {doc_block}
            inst = cls.__new__(cls)
            with catch_and_log_exceptions(context=cls.__name__):
                kwargs = {{
                    {kwargs},
                }}

                if clear_unset:
                    kwargs = {{k: v if v is not None else [] for k, v in kwargs.items()}}  # type: ignore[misc]

                inst.__attrs_init__(**kwargs)
                return inst

            inst.__attrs_clear__()
            return inst

        @classmethod
        def cleared(cls) -> {name}:
            """Clear all the fields of a `{name}`."""
            return cls.from_fields(clear_unset=True)
        "#,
        extra_decorators = classmethod_decorators(obj)
    ))
}

pub fn quote_columnar_methods(reporter: &Reporter, obj: &Object, objects: &Objects) -> String {
    let parameters = obj
        .fields
        .iter()
        .filter_map(|field| {
            let mut field = field.make_plural()?;
            field.is_nullable = true;
            Some(quote_init_parameter_from_field(
                &field,
                objects,
                &obj.fqname,
            ))
        })
        .collect_vec()
        .join(",\n");
    let parameters = indent::indent_by(8, parameters);

    let init_args = obj
        .fields
        .iter()
        .map(|field| {
            let field_name = field.snake_case_name();
            format!("{field_name}={field_name}")
        })
        .collect_vec()
        .join(",\n");
    let init_args = indent::indent_by(12, init_args);

    let parameter_docs = compute_init_parameter_docs(reporter, obj, objects);
    let doc = unindent(
        "\
        Construct a new column-oriented component bundle.

        This makes it possible to use `rr.send_columns` to send columnar data directly into Rerun.

        The returned columns will be partitioned into unit-length sub-batches by default.
        Use `ComponentColumnList.partition` to repartition the data as needed.
        ",
    );
    let mut doc_string_lines = doc.lines().map(|s| s.to_owned()).collect_vec();
    if !parameter_docs.is_empty() {
        doc_string_lines.push("Parameters".to_owned());
        doc_string_lines.push("----------".to_owned());
        for doc in parameter_docs {
            doc_string_lines.push(doc);
        }
    }
    let doc_block = indent::indent_by(12, quote_doc_lines(doc_string_lines));

    let kwargs = quote_component_field_mapping(obj);
    let kwargs = indent::indent_by(12, kwargs);

    // NOTE: Calling `update_fields` is not an option: we need to be able to pass
    // plural data, even to singular fields (mono-components).
    unindent(&format!(
        r#"
        @classmethod
        {extra_decorators}
        def columns(
            cls,
            *,
            {parameters},
        ) -> ComponentColumnList:
            {doc_block}
            inst = cls.__new__(cls)
            with catch_and_log_exceptions(context=cls.__name__):
                inst.__attrs_init__(
                    {init_args},
                )

            batches = inst.as_component_batches()
            if len(batches) == 0:
                return ComponentColumnList([])

            kwargs = {{
                {kwargs}
            }}
            columns = []

            for batch in batches:
                arrow_array = batch.as_arrow_array()

                # For primitive arrays and fixed size list arrays, we infer partition size from the input shape.
                if pa.types.is_primitive(arrow_array.type) or pa.types.is_fixed_size_list(arrow_array.type):
                    param = kwargs[batch.component_descriptor().component] # type: ignore[index]
                    shape = np.shape(param)  # type: ignore[arg-type]
                    elem_flat_len = int(np.prod(shape[1:])) if len(shape) > 1 else 1  # type: ignore[redundant-expr,misc]

                    if pa.types.is_fixed_size_list(arrow_array.type) and arrow_array.type.list_size == elem_flat_len:
                        # If the product of the last dimensions of the shape are equal to the size of the fixed size list array,
                        # we have `num_rows` single element batches (each element is a fixed sized list).
                        # (This should have been already validated by conversion to the arrow_array)
                        batch_length = 1
                    else:
                        batch_length = shape[1] if len(shape) > 1 else 1  # type: ignore[redundant-expr,misc]

                    num_rows = shape[0] if len(shape) >= 1 else 1  # type: ignore[redundant-expr,misc]
                    sizes = batch_length * np.ones(num_rows)
                else:
                    # For non-primitive types, default to partitioning each element separately.
                    sizes = np.ones(len(arrow_array))

                columns.append(batch.partition(sizes))

            return ComponentColumnList(columns)
        "#,
        extra_decorators = classmethod_decorators(obj)
    ))
}
