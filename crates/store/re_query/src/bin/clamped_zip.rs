//! CLI tool to generate `ClampedZip` implementations of different arities.

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

    /// `r0: R0, r1: R1, r2: R2…`.
    fn to_required_params(&self) -> Vec<String> {
        izip!(self.to_required_names(), self.to_required_types())
            .map(|(n, t)| format!("{n}: {t}"))
            .collect()
    }

    /// `R0: (Into)Iterator, R1: (Into)Iterator, R2: (Into)Iterator…`
    fn to_required_clauses(&self, into: bool) -> Vec<String> {
        let trait_name = if into { "IntoIterator" } else { "Iterator" };
        self.to_required_types()
            .into_iter()
            .map(|t| format!("{t}: {trait_name}"))
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

    /// `o0: O0, o1: O1, o2: O2…`.
    fn to_optional_params(&self) -> Vec<String> {
        izip!(self.to_optional_names(), self.to_optional_types())
            .map(|(n, t)| format!("{n}: {t}"))
            .collect()
    }

    /// `O0: IntoIterator, O0::Item: Clone, O1: IntoIterator, O1::Item: Clone…`
    fn to_optional_clauses(&self, into: bool) -> Vec<String> {
        let trait_name = if into { "IntoIterator" } else { "Iterator" };
        self.to_optional_types()
            .into_iter()
            .map(|t| format!("{t}: {trait_name}, {t}::Item: Clone"))
            .collect()
    }

    /// `o0_default_fn, o1_default_fn, o2_default_fn…`.
    fn to_optional_fn_names(&self) -> Vec<String> {
        (0..self.num_optional)
            .map(|n| format!("o{n}_default_fn"))
            .collect_vec()
    }

    /// `D0, D1, D2…`.
    fn to_optional_fn_types(&self) -> Vec<String> {
        (0..self.num_optional)
            .map(|n| format!("D{n}"))
            .collect_vec()
    }

    /// `o0_default_fn: D0, o1_default_fn: D1…`.
    fn to_optional_fn_params(&self) -> Vec<String> {
        izip!(self.to_optional_fn_names(), self.to_optional_fn_types())
            .map(|(n, t)| format!("{n}: {t}"))
            .collect()
    }

    /// `D0: Fn() -> O0::Item, D1: Fn() -> O1::Item…`
    fn to_optional_fn_clauses(&self) -> Vec<String> {
        izip!(self.to_optional_fn_types(), self.to_optional_types())
            .map(|(tl, tr)| format!("{tl}: Fn() -> {tr}::Item"))
            .collect()
    }
}

fn backticked(strs: impl IntoIterator<Item = String>) -> Vec<String> {
    strs.into_iter().map(|s| format!("`{s}`")).collect()
}

fn generate_helper_func(params: &Params) -> String {
    let suffix = params.to_suffix();
    let required_names = backticked(params.to_required_names()).join(", ");
    let optional_names = backticked(params.to_optional_names()).join(", ");
    let optional_fn_names = backticked(params.to_optional_fn_names()).join(", ");
    let required_types = params.to_required_types().join(", ");
    let optional_types = params.to_optional_types().join(", ");
    let optional_fn_types = params.to_optional_fn_types().join(", ");
    let required_clauses = params.to_required_clauses(true /* into */).join(", ");
    let optional_clauses = params.to_optional_clauses(true /* into */).join(", ");
    let optional_fn_clauses = params.to_optional_fn_clauses().join(", ");
    let required_params = params.to_required_params().join(", ");
    let optional_params = izip!(params.to_optional_params(), params.to_optional_fn_params())
        .map(|(o, d)| format!("{o}, {d}"))
        .collect_vec()
        .join(",\n");

    let ret_clause = params
        .to_required_types()
        .into_iter()
        .map(|r| format!("{r}::IntoIter"))
        .chain(
            params
                .to_optional_types()
                .into_iter()
                .map(|o| format!("{o}::IntoIter")),
        )
        .chain(params.to_optional_fn_types())
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
                .map(|o| format!("{o}: {o}.into_iter()")),
        )
        .chain(params.to_optional_fn_names())
        .chain(
            params
                .to_optional_names()
                .into_iter()
                .map(|o| format!("{o}_latest_value: None")),
        )
        .collect_vec()
        .join(",\n");

    format!(
        r#"
        /// Returns a new [`ClampedZip{suffix}`] iterator.
        ///
        /// The number of elements in a clamped zip iterator corresponds to the number of elements in the
        /// shortest of its required iterators ({required_names}).
        ///
        /// Optional iterators ({optional_names}) will repeat their latest values if they happen to be too short
        /// to be zipped with the shortest of the required iterators.
        ///
        /// If an optional iterator is not only too short but actually empty, its associated default function
        /// ({optional_fn_names}) will be executed and the resulting value repeated as necessary.
        pub fn clamped_zip_{suffix}<{required_types}, {optional_types}, {optional_fn_types}>(
            {required_params},
            {optional_params},
        ) -> ClampedZip{suffix}<{ret_clause}>
        where
            {required_clauses},
            {optional_clauses},
            {optional_fn_clauses},
        {{
            ClampedZip{suffix} {{
                {ret}
            }}
        }}
    "#
    )
}

fn generate_struct(params: &Params) -> String {
    let suffix = params.to_suffix();
    let required_types = params.to_required_types().join(", ");
    let optional_types = params.to_optional_types().join(", ");
    let optional_fn_types = params.to_optional_fn_types().join(", ");
    let required_clauses = params.to_required_clauses(false /* into */).join(", ");
    let optional_clauses = params.to_optional_clauses(false /* into */).join(", ");
    let optional_fn_clauses = params.to_optional_fn_clauses().join(", ");
    let required_params = params.to_required_params().join(", ");
    let optional_params = params.to_optional_params().join(", ");
    let optional_fn_params = params.to_optional_fn_params().join(", ");

    let latest_values = izip!(params.to_optional_names(), params.to_optional_types())
        .map(|(n, t)| format!("{n}_latest_value: Option<{t}::Item>"))
        .collect_vec()
        .join(",\n");

    format!(
        r#"
        /// Implements a clamped zip iterator combinator with 2 required iterators and 2 optional
        /// iterators.
        ///
        /// See [`clamped_zip_{suffix}`] for more information.
        pub struct ClampedZip{suffix}<{required_types}, {optional_types}, {optional_fn_types}>
        where
            {required_clauses},
            {optional_clauses},
            {optional_fn_clauses},
        {{
            {required_params},
            {optional_params},
            {optional_fn_params},

            {latest_values}
        }}
    "#
    )
}

fn generate_impl(params: &Params) -> String {
    let suffix = params.to_suffix();
    let required_types = params.to_required_types().join(", ");
    let optional_types = params.to_optional_types().join(", ");
    let optional_fn_types = params.to_optional_fn_types().join(", ");
    let required_clauses = params.to_required_clauses(false /* into */).join(", ");
    let optional_clauses = params.to_optional_clauses(false /* into */).join(", ");
    let optional_fn_clauses = params.to_optional_fn_clauses().join(", ");

    let items = params
        .to_required_types()
        .into_iter()
        .map(|r| format!("{r}::Item"))
        .chain(
            params
                .to_optional_types()
                .into_iter()
                .map(|o| format!("{o}::Item")),
        )
        .collect_vec()
        .join(", ");

    let next =
        params
            .to_required_names()
            .into_iter()
            .map(|r| format!("let {r}_next = self.{r}.next()?;"))
            .chain(params.to_optional_names().into_iter().map(|o| {
                format!("let {o}_next = self.{o}.next().or(self.{o}_latest_value.take());")
            }))
            .collect_vec()
            .join("\n");

    let update_latest = params
        .to_optional_names()
        .into_iter()
        .map(|o| format!("self.{o}_latest_value.clone_from(&{o}_next);"))
        .collect_vec()
        .join("\n");

    let ret = params
        .to_required_names()
        .into_iter()
        .map(|r| format!("{r}_next"))
        .chain(
            params
                .to_optional_names()
                .into_iter()
                .map(|o| format!("{o}_next.unwrap_or_else(|| (self.{o}_default_fn)())")),
        )
        .collect_vec()
        .join(",\n");

    format!(
        r#"
        impl<{required_types}, {optional_types}, {optional_fn_types}> Iterator for ClampedZip{suffix}<{required_types}, {optional_types}, {optional_fn_types}>
        where
            {required_clauses},
            {optional_clauses},
            {optional_fn_clauses},
        {{
            type Item = ({items});

            #[inline]
            fn next(&mut self) -> Option<Self::Item> {{
                {next}

                {update_latest}

                Some((
                    {ret}
                ))
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
        // This file was generated using `cargo r -p re_query --all-features --bin clamped_zip`.
        // DO NOT EDIT.

        // ---

        #![expect(clippy::too_many_arguments)]
        #![expect(clippy::type_complexity)]

        {output}
        "
    );
}
