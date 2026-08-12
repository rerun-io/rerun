//! Url decoration callback. This exists so we can show rich data that cannot be solely derived
//! from just the name (e.g. resolving the entry id to the entry name) `data_label` doesn't have access to our.

use crate::LinkButton;

/// A decorator that turns a recognized URL string into a labeled [`LinkButton`].
pub type UrlDecoratorFn = std::sync::Arc<dyn Fn(&str) -> Option<LinkButton> + Send + Sync>;

/// The globally-installed [`UrlDecoratorFn`], stored in egui memory.
#[derive(Clone)]
pub struct UrlDecorator(UrlDecoratorFn);

fn url_decorator_id() -> egui::Id {
    egui::Id::new("re_ui::url_decorator")
}

impl UrlDecorator {
    /// Install it
    pub fn set(
        ctx: &egui::Context,
        decorator: impl Fn(&str) -> Option<LinkButton> + Send + Sync + 'static,
    ) {
        let decorator: UrlDecoratorFn = std::sync::Arc::new(decorator);
        ctx.data_mut(|data| {
            data.insert_temp(url_decorator_id(), Self(decorator));
        });
    }

    pub fn get(ctx: &egui::Context) -> Option<UrlDecoratorFn> {
        ctx.data(|data| data.get_temp::<Self>(url_decorator_id()))
            .map(|installed| installed.0)
    }
}
