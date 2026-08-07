// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// An individual query expression used to filter a set of [datatypes.EntityPath]s.
///
/// Each expression is either an inclusion or an exclusion expression.
/// Inclusions start with an optional `+` and exclusions must start with a `-`.
///
/// Multiple expressions are combined together as part of [archetypes.ViewContents].
///
/// The `/**` suffix matches the whole subtree, i.e. self and any child, recursively
/// (`/world/**` matches both `/world` and `/world/car/driver`).
/// Other uses of `*` are not (yet) supported.
#[rerun::rerun_type]
#[python(aliases = "str")]
#[python(array_aliases = "str | Sequence[str]")]
#[rerun(scope = "blueprint")]
#[rust(derive(Default, PartialEq, Eq, PartialOrd, Ord))]
#[rust(repr = "transparent")]
#[rerun(state = "unstable")]
pub struct QueryExpression {
    pub filter: rerun::datatypes::Utf8,
}
