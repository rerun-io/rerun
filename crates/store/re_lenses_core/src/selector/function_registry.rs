use std::sync::Arc;

use arrow::array::ArrayRef;

use super::Literal;

/// A shared, type-erased function operating on Arrow arrays.
///
/// Uses `Arc` so that `DynExpr` can derive `Clone`, which is needed by language bindings like `PyO3`.
pub type BoxedFunction =
    Arc<dyn Fn(&ArrayRef) -> Result<Option<ArrayRef>, crate::combinators::Error> + Send + Sync>;

/// A constructor that creates a [`BoxedFunction`] from a list of arguments.
type BoxedFunctionConstructor = Box<dyn Fn(&[Literal]) -> Option<BoxedFunction> + Send + Sync>;

/// Errors that can occur when working with the function registry.
#[derive(Clone, Debug, thiserror::Error)]
pub enum FunctionRegistryError {
    #[error("Duplicate function registered: `{name}`")]
    DuplicateFunction { name: String },

    #[error("Unknown function: `{name}`")]
    UnknownFunction { name: String },

    #[error("Wrong arguments for function: `{name}`")]
    WrongArguments { name: String },
}

/// A registry of named function constructors.
///
/// Functions are registered by name along with a constructor that takes
/// arguments and produces a concrete [`BoxedFunction`] implementation. This
/// allows referencing functions by name and instantiate them at runtime.
pub struct FunctionRegistry {
    constructors: ahash::HashMap<String, BoxedFunctionConstructor>,
}

impl re_byte_size::SizeBytes for FunctionRegistry {
    fn heap_size_bytes(&self) -> u64 {
        let Self { constructors } = self;

        // Can't know internal heap size of the type erased function constructor, so assume it's
        // zero.
        constructors.capacity() as u64
            * (std::mem::size_of::<String>() + std::mem::size_of::<BoxedFunctionConstructor>())
                as u64
            + constructors
                .keys()
                .map(|s| s.heap_size_bytes())
                .sum::<u64>()
    }
}

impl FunctionRegistry {
    pub fn new() -> Self {
        Self {
            constructors: ahash::HashMap::default(),
        }
    }

    /// Register a function constructor under the given name.
    #[inline]
    pub fn register<Args, F: FunctionConstructor<Args>>(
        &mut self,
        name: impl Into<String>,
        f: F,
    ) -> Result<(), FunctionRegistryError> {
        use std::collections::hash_map::Entry;
        match self.constructors.entry(name.into()) {
            Entry::Occupied(entry) => Err(FunctionRegistryError::DuplicateFunction {
                name: entry.key().clone(),
            }),
            Entry::Vacant(entry) => {
                entry.insert(Box::new(move |arguments| f.constructor(arguments)));

                Ok(())
            }
        }
    }

    /// Instantiate a function by name with the given arguments.
    pub fn get(
        &self,
        name: &str,
        args: &[Literal],
    ) -> Result<BoxedFunction, FunctionRegistryError> {
        let constructor =
            self.constructors
                .get(name)
                .ok_or_else(|| FunctionRegistryError::UnknownFunction {
                    name: name.to_owned(),
                })?;
        constructor(args).ok_or_else(|| FunctionRegistryError::WrongArguments {
            name: name.to_owned(),
        })
    }
}

impl Default for FunctionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

trait FromLiteral: Sized {
    fn from_literal(literal: &Literal) -> Option<Self>;
}

impl FromLiteral for String {
    fn from_literal(literal: &Literal) -> Option<Self> {
        match literal {
            Literal::String(s) => Some(s.clone()),
        }
    }
}

pub trait FunctionConstructor<Args>: Send + Sync + 'static {
    fn constructor(&self, arguments: &[Literal]) -> Option<BoxedFunction>;
}

macro_rules! impl_function_constructors {
    () => {
        impl_function_constructors!(impl);
    };
    (impl $($ident:ident)*) => {
        #[expect(clippy::allow_attributes)]
        #[allow(unused_parens)]
        impl<
            $($ident: FromLiteral,)*
            T: Fn(&ArrayRef) -> Result<Option<ArrayRef>, crate::combinators::Error> + Send + Sync + 'static,
            F: Fn($($ident),*) -> T + Send + Sync + 'static,
        > FunctionConstructor<($($ident),*)> for F {
            fn constructor(&self, arguments: &[Literal]) -> Option<BoxedFunction> {
                let mut _args = arguments.iter();
                let t = (self)(
                    $(
                        $ident::from_literal(_args.next()?)?,
                    )*
                );

                Some(Arc::new(t))
            }
        }
    };
    ($head:ident $($tail:ident)*) => {
        impl_function_constructors!($($tail)*);
        impl_function_constructors!(impl $head $($tail)*);
    };
}

impl_function_constructors!(T0 T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12);
