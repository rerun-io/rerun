use crate::function_registry::BoxedFunction;

use super::{Selector, parser::Expr};

/// A dynamic expression that extends `Expr` with support for anonymous functions.
///
/// Unlike `Expr`, a `DynExpr` cannot implement [`Display`](std::fmt::Display) because
/// anonymous functions are not serializable.
pub enum DynExpr {
    /// Delegate to a static `Expr`.
    Expr(Expr),

    /// Pipe two `DynExpr`s together (left then right).
    Pipe { left: Box<Self>, right: Box<Self> },

    /// An anonymous (unregistered) function.
    Function(BoxedFunction),
}

impl From<Expr> for DynExpr {
    fn from(expr: Expr) -> Self {
        Self::Expr(expr)
    }
}

impl<E: Into<Self>> From<Selector<E>> for DynExpr {
    fn from(selector: Selector<E>) -> Self {
        selector.expr.into()
    }
}

impl std::fmt::Debug for DynExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Expr(expr) => f.debug_tuple("Expr").field(expr).finish(),
            Self::Pipe { left, right } => f
                .debug_struct("Pipe")
                .field("left", left)
                .field("right", right)
                .finish(),
            Self::Function(_) => f.debug_tuple("Function").field(&"<dyn>").finish(),
        }
    }
}

impl std::fmt::Debug for Selector<DynExpr> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { expr } = self;

        f.debug_struct("Selector").field("expr", expr).finish()
    }
}
