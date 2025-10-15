//! CLI tool to generate `RangeZip` implementations of different arities.

#![expect(clippy::tuple_array_conversions)] // false positive

use itertools::{Itertools as _, izip};

struct Params {
    num_required: usize,
    num_optional: usize,
}

impl Params {
    fn to_num_required(&self) -> String {
        self.num_required.to_string()
    }

    fn to_num_optional(&self) -> String {
        self.num_optional.to_string()
    }

    /// `1x3`, `2x2`…
    fn to_suffix(&self) -> String {
        format!("{}x{}", self.to_num_required(), self.to_num_optional())
    }

    /// `r0, r1, r2…`.
    fn to_required_names(&self) -> Vec<String> {
        (0..self.num_required)
            .map(|n| format!("r{n}"))
            .collect_vec()
    }

    /// `R0, R1, R2…`.
    fn to_required_types(&self) -> Vec<String> {
        self.to_required_names()
            .into_iter()
            .map(|s| s.to_uppercase())
            .collect()
    }

    /// `r0: IR0, r1: IR1, r2: IR2…`.
    fn to_required_params(&self) -> Vec<String> {
        izip!(self.to_required_names(), self.to_required_types())
            .map(|(n, t)| format!("{n}: I{t}"))
            .collect()
    }

    /// `IR0: (Into)Iterator<Item = (Idx, R0)>, IR1: (Into)Iterator<Item = (Idx, R1)>…`
    fn to_required_clauses(&self, into: bool) -> Vec<String> {
        let trait_name = if into { "IntoIterator" } else { "Iterator" };
        self.to_required_types()
            .into_iter()
            .map(|t| format!("I{t}: {trait_name}<Item = (Idx, {t})>"))
            .collect()
    }

    /// `o0, o1, o2…`.
    fn to_optional_names(&self) -> Vec<String> {
        (0..self.num_optional)
            .map(|n| format!("o{n}"))
            .collect_vec()
    }

    /// `O0, O1, O2…`.
    fn to_optional_types(&self) -> Vec<String> {
        self.to_optional_names()
            .into_iter()
            .map(|s| s.to_uppercase())
            .collect()
    }

    /// `o0: IO0, o1: IO1, o2: IO2…`.
    fn to_optional_params(&self) -> Vec<String> {
        izip!(self.to_optional_names(), self.to_optional_types())
            .map(|(n, t)| format!("{n}: I{t}"))
            .collect()
    }

    /// `o0: Peekable<IO0>, o1: Peekable<IO1>, o2: Peekable<IO2>…`.
    fn to_optional_peekable_params(&self) -> Vec<String> {
        izip!(self.to_optional_names(), self.to_optional_types())
            .map(|(n, t)| format!("{n}: Peekable<I{t}>"))
            .collect()
    }

    /// `IO0: (Into)Iterator<Item = (Idx, O0)>, IO1: (Into)Iterator<Item = (Idx, O1)>…`
    fn to_optional_clauses(&self, into: bool) -> Vec<String> {
        let trait_name = if into { "IntoIterator" } else { "Iterator" };
        self.to_optional_types()
            .into_iter()
            .map(|t| format!("I{t}: {trait_name}<Item = (Idx, {t})>"))
            .collect()
    }
}

fn backticked(strs: impl IntoIterator<Item = String>) -> Vec<String> {
    strs.into_iter().map(|s| format!("`{s}`")).collect()
}

/// Output:
/// ```ignore
/// pub fn range_zip_2x2<Idx, IR0, R0, IR1, R1, IO0, O0, IO1, O1>(
///     r0: IR0,
///     r1: IR1,
///     o0: IO0,
///     o1: IO1,
/// ) -> RangeZip2x2<Idx, IR0::IntoIter, R0, IR1::IntoIter, R1, IO0::IntoIter, O0, IO1::IntoIter, O1>
/// where
///     Idx: std::cmp::Ord,
///     IR0: IntoIterator<Item = (Idx, R0)>,
///     IR1: IntoIterator<Item = (Idx, R1)>,
///     IO0: IntoIterator<Item = (Idx, O0)>,
///     IO1: IntoIterator<Item = (Idx, O1)>,
/// {
///     RangeZip2x2 {
///         r0: r0.into_iter(),
///         r1: r1.into_iter(),
///         o0: o0.into_iter().peekable(),
///         o1: o1.into_iter().peekable(),
///
///         o0_data_latest: None,
///         o1_data_latest: None,
///     }
/// }
/// ```
fn generate_helper_func(params: &Params) -> String {
    let suffix = params.to_suffix();
    let required_names = backticked(params.to_required_names()).join(", ");
    let required_types = izip!(
        params
            .to_required_types()
            .into_iter()
            .map(|t| format!("I{t}")),
        params.to_required_types()
    )
    .flat_map(|(tr, r)| [tr, r])
    .collect_vec()
    .join(", ");
    let optional_types = izip!(
        params
            .to_optional_types()
            .into_iter()
            .map(|t| format!("I{t}")),
        params.to_optional_types()
    )
    .flat_map(|(tr, r)| [tr, r])
    .collect_vec()
    .join(", ");
    let required_clauses = params.to_required_clauses(true /* into */).join(", ");
    let optional_clauses = params.to_optional_clauses(true /* into */).join(", ");
    let required_params = params.to_required_params().join(", ");
    let optional_params = params.to_optional_params().join(", ");

    let ret_clause = params
        .to_required_types()
        .into_iter()
        .map(|r| format!("I{r}::IntoIter, {r}"))
        .chain(
            params
                .to_optional_types()
                .into_iter()
                .map(|o| format!("I{o}::IntoIter, {o}")),
        )
        .collect_vec()
        .join(", ");

    let ret = params
        .to_required_names()
        .into_iter()
        .map(|r| format!("{r}: {r}.into_iter()"))
        .chain(
            params
                .to_optional_names()
                .into_iter()
                .map(|o| format!("{o}: {o}.into_iter().peekable()")),
        )
        .collect_vec()
        .join(",\n");

    let latest = params
        .to_optional_names()
        .into_iter()
        .map(|o| format!("{o}_data_latest: None"))
        .collect_vec()
        .join(",\n");

    format!(
        r#"
        /// Returns a new [`RangeZip{suffix}`] iterator.
        ///
        /// The number of elements in a range zip iterator corresponds to the number of elements in the
        /// shortest of its required iterators ({required_names}).
        ///
        /// Each call to `next` is guaranteed to yield the next value for each required iterator,
        /// as well as the most recent index amongst all of them.
        ///
        /// Optional iterators accumulate their state and yield their most recent value (if any),
        /// each time the required iterators fire.
        pub fn range_zip_{suffix}<Idx, {required_types}, {optional_types}>(
            {required_params},
            {optional_params},
        ) -> RangeZip{suffix}<Idx, {ret_clause}>
        where
            Idx: std::cmp::Ord,
            {required_clauses},
            {optional_clauses},
        {{
            RangeZip{suffix} {{
                {ret},

                {latest},
            }}
        }}
    "#
    )
}

/// Output:
/// ```ignore
/// pub struct RangeZip2x2<Idx, IR0, R0, IR1, R1, IO0, O0, IO1, O1>
/// where
///     Idx: std::cmp::Ord,
///     IR0: Iterator<Item = (Idx, R0)>,
///     IR1: Iterator<Item = (Idx, R1)>,
///     IO0: Iterator<Item = (Idx, O0)>,
///     IO1: Iterator<Item = (Idx, O1)>,
/// {
///     r0: IR0,
///     r1: IR1,
///     o0: Peekable<IO0>,
///     o1: Peekable<IO1>,
///
///     o0_data_latest: Option<O0>,
///     o1_data_latest: Option<O1>,
/// }
/// ```
fn generate_struct(params: &Params) -> String {
    let suffix = params.to_suffix();
    let required_types = izip!(
        params
            .to_required_types()
            .into_iter()
            .map(|t| format!("I{t}")),
        params.to_required_types()
    )
    .flat_map(|(tr, r)| [tr, r])
    .collect_vec()
    .join(", ");
    let optional_types = izip!(
        params
            .to_optional_types()
            .into_iter()
            .map(|t| format!("I{t}")),
        params.to_optional_types()
    )
    .flat_map(|(tr, r)| [tr, r])
    .collect_vec()
    .join(", ");
    let required_clauses = params.to_required_clauses(false /* into */).join(", ");
    let optional_clauses = params.to_optional_clauses(false /* into */).join(", ");
    let required_params = params.to_required_params().join(", ");
    let optional_params = params.to_optional_peekable_params().join(", ");
    let optional_latest_params = izip!(params.to_optional_names(), params.to_optional_types())
        .map(|(n, t)| format!("{n}_data_latest: Option<{t}>"))
        .join(", ");

    format!(
        r#"
        /// Implements a range zip iterator combinator with 2 required iterators and 2 optional
        /// iterators.
        ///
        /// See [`range_zip_{suffix}`] for more information.
        pub struct RangeZip{suffix}<Idx, {required_types}, {optional_types}>
        where
            Idx: std::cmp::Ord,
            {required_clauses},
            {optional_clauses},
        {{
            {required_params},
            {optional_params},

            {optional_latest_params},
        }}
    "#
    )
}

/// Output:
/// ```ignore
/// impl<Idx, IR0, R0, IR1, R1, IO0, O0, IO1, O1> Iterator
///     for RangeZip2x2<Idx, IR0, R0, IR1, R1, IO0, O0, IO1, O1>
/// where
///     Idx: std::cmp::Ord,
///     IR0: Iterator<Item = (Idx, R0)>,
///     IR1: Iterator<Item = (Idx, R1)>,
///     IO0: Iterator<Item = (Idx, O0)>,
///     IO1: Iterator<Item = (Idx, O1)>,
///     O0: Clone,
///     O1: Clone,
/// {
///     type Item = (Idx, R0, R1, Option<O0>, Option<O1>);
///
///     #[inline]
///     fn next(&mut self) -> Option<Self::Item> {
///         let Self {
///             r0,
///             r1,
///             o0,
///             o1,
///             o0_data_latest,
///             o1_data_latest,
///         } = self;
///
///         let (r0_index, r0_data) = r0.next()?;
///         let (r1_index, r1_data) = r1.next()?;
///
///         let max_index = [r0_index, r1_index].into_iter().max()?;
///
///         let mut o0_data = None;
///         while let Some((_, data)) = o0.next_if(|(index, _)| index <= &max_index) {
///             o0_data = Some(data);
///         }
///         let o0_data = o0_data.or(o0_data_latest.take());
///         o0_data_latest.clone_from(&o0_data);
///
///         let mut o1_data = None;
///         while let Some((_, data)) = o1.next_if(|(index, _)| index <= &max_index) {
///             o1_data = Some(data);
///         }
///         let o1_data = o1_data.or(o1_data_latest.take());
///         o1_data_latest.clone_from(&o1_data);
///
///         Some((max_index, r0_data, r1_data, o0_data, o1_data))
///     }
/// }
/// ```
fn generate_impl(params: &Params) -> String {
    let suffix = params.to_suffix();
    let required_types = izip!(
        params
            .to_required_types()
            .into_iter()
            .map(|t| format!("I{t}")),
        params.to_required_types()
    )
    .flat_map(|(tr, r)| [tr, r])
    .collect_vec()
    .join(", ");
    let optional_types = izip!(
        params
            .to_optional_types()
            .into_iter()
            .map(|t| format!("I{t}")),
        params.to_optional_types()
    )
    .flat_map(|(tr, r)| [tr, r])
    .collect_vec()
    .join(", ");
    let required_names = params.to_required_names().join(", ");
    let optional_names = params.to_optional_names().join(", ");
    let optional_latest_names = params
        .to_optional_names()
        .into_iter()
        .map(|n| format!("{n}_data_latest"))
        .join(", ");
    let required_indices = params
        .to_required_names()
        .into_iter()
        .map(|n| format!("{n}_index"))
        .collect_vec()
        .join(", ");
    let required_data = params
        .to_required_names()
        .into_iter()
        .map(|n| format!("{n}_data"))
        .collect_vec()
        .join(", ");
    let optional_data = params
        .to_optional_names()
        .into_iter()
        .map(|n| format!("{n}_data"))
        .collect_vec()
        .join(", ");
    let required_clauses = params.to_required_clauses(false /* into */).join(", ");
    let optional_clauses = params.to_optional_clauses(false /* into */).join(", ");
    let optional_clone_clauses = params
        .to_optional_types()
        .into_iter()
        .map(|o| format!("{o}: Clone"))
        .collect_vec()
        .join(", ");

    let items = params
        .to_required_types()
        .into_iter()
        .chain(
            params
                .to_optional_types()
                .into_iter()
                .map(|o| format!("Option<{o}>")),
        )
        .collect_vec()
        .join(", ");

    let next_required = params
        .to_required_names()
        .into_iter()
        .map(|r| format!("let ({r}_index, {r}_data) = {r}.next()?;"))
        .collect_vec()
        .join("\n");

    let next_optional = params
        .to_optional_names()
        .into_iter()
        .map(|o| {
            format!(
                "
                let mut {o}_data = None;
                while let Some((_, data)) = {o}.next_if(|(index, _)| index <= &max_index) {{
                    {o}_data = Some(data);
                }}
                let {o}_data = {o}_data.or({o}_data_latest.take());
                {o}_data_latest.clone_from(&{o}_data);
                "
            )
        })
        .collect_vec()
        .join("\n");

    format!(
        r#"
        impl<Idx, {required_types}, {optional_types}> Iterator for RangeZip{suffix}<Idx, {required_types}, {optional_types}>
        where
            Idx: std::cmp::Ord,
            {required_clauses},
            {optional_clauses},
            {optional_clone_clauses},
        {{
            type Item = (Idx, {items});

            #[inline]
            fn next(&mut self) -> Option<Self::Item> {{
                let Self {{ {required_names}, {optional_names}, {optional_latest_names} }} = self;

                {next_required}

                let max_index = [{required_indices}].into_iter().max()?;

                {next_optional}

                Some((max_index, {required_data}, {optional_data}))
            }}
        }}
    "#
    )
}

fn main() {
    let num_required = 1..3;
    let num_optional = 1..10;

    let output = num_required
        .flat_map(|num_required| {
            num_optional
                .clone()
                .map(move |num_optional| (num_required, num_optional))
        })
        .flat_map(|(num_required, num_optional)| {
            let params = Params {
                num_required,
                num_optional,
            };

            [
                generate_helper_func(&params),
                generate_struct(&params),
                generate_impl(&params),
            ]
        })
        .collect_vec()
        .join("\n");

    println!(
        "
        // This file was generated using `cargo r -p re_query --all-features --bin range_zip`.
        // DO NOT EDIT.

        // ---

        #![expect(clippy::iter_on_single_items)]
        #![expect(clippy::too_many_arguments)]
        #![expect(clippy::type_complexity)]

        use std::iter::Peekable;

        {output}
        "
    );
}
