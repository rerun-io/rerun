use egui::{Popup, Response, Widget, WidgetText};

type ResponseCallbacks<'a> = smallvec::SmallVec<[Box<dyn FnOnce(Response) -> Response + 'a>; 1]>;

/// Wrap a [`Widget`] and register actions on its [`Response`].
pub struct OnResponse<'a, T> {
    inner: T,
    on_response: ResponseCallbacks<'a>,
    enabled: bool,
}

/// Ideally we'd implement something like `impl<'a, T> OnResponseExt<'a> for OnResponse<'a, T>`
/// but we can't since we have a blanket impl for all `T: Widget` and `OnResponse` also implements
/// `Widget`. So a macro will have to do.
///
/// We could also remove the implementations for `OnResponse` and most code should still work,
/// but then every further call to one of the `OnResponseExt` methods would introduce another layer of
/// `OnResponse<OnResponse<OnResponse<T>>>`, which would be annoying.
/// On the other hand, maybe that would be nice and more efficient?
/// We could remove the smallvec and instead be generic over the Widget and an `FnOnce`.
macro_rules! response_ext_impl {
    ($target:path, $($pub:tt)*) => {
        /// Enable / disable the widget.
        #[inline]
        $($pub)* fn enabled(self, enabled: bool) -> OnResponse<'a, $target> {
            let mut on_response = self.into_on_response();
            on_response.enabled = enabled;
            on_response
        }

        /// Add a callback that is called with the response of the widget once it's added.
        #[inline]
        $($pub)* fn on_response(
            self,
            on_response: impl FnOnce(Response) -> Response + 'a,
        ) -> OnResponse<'a, $target> {
            let mut wrapped = self.into_on_response();
            wrapped.on_response.push(Box::new(on_response));
            wrapped
        }

        /// Add a callback that is called when the widget is clicked.
        #[inline]
        $($pub)* fn on_click(self, on_click: impl FnOnce() + 'a) -> OnResponse<'a, $target> {
            self.on_response(move |response| {
                if response.clicked() {
                    on_click();
                }
                response
            })
        }

        /// Add some tooltip UI to the widget.
        #[inline]
        $($pub)* fn on_hover_ui(
            self,
            on_hover_ui: impl FnOnce(&mut egui::Ui) + 'a,
        ) -> OnResponse<'a, $target> {
            self.on_response(move |response| response.on_hover_ui(on_hover_ui))
        }

        /// Add some tooltip UI to the widget when it's disabled.
        #[inline]
        $($pub)* fn on_disabled_hover_ui(
            self,
            on_hover_ui: impl FnOnce(&mut egui::Ui) + 'a,
        ) -> OnResponse<'a, $target> {
            self.on_response(move |response| response.on_disabled_hover_ui(on_hover_ui))
        }

        /// Add some tooltip text to the widget.
        #[inline]
        $($pub)* fn on_hover_text(self, hover_text: impl Into<WidgetText> + 'a) -> OnResponse<'a, $target> {
            let hover_text = hover_text.into();
            self.on_response(move |response| response.on_hover_text(hover_text.clone()))
        }

        /// Add some tooltip text to the widget when it's disabled.
        #[inline]
        $($pub)* fn on_disabled_hover_text(
            self,
            hover_text: impl Into<WidgetText> + 'a,
        ) -> OnResponse<'a, $target> {
            let hover_text = hover_text.into();
            self.on_response(move |response| response.on_disabled_hover_text(hover_text.clone()))
        }

        /// Show a menu on click.
        #[inline]
        $($pub)* fn on_menu(self, add_contents: impl FnOnce(&mut egui::Ui) + 'a) -> OnResponse<'a, $target> {
            self.on_custom_menu(|popup| popup, add_contents)
        }

        /// Show a custom menu on click.
        #[inline]
        $($pub)* fn on_custom_menu(
            self,
            customize: impl FnOnce(Popup<'_>) -> Popup<'_> + 'a,
            add_contents: impl FnOnce(&mut egui::Ui) + 'a,
        ) -> OnResponse<'a, $target> {
            self.on_response(move |response| {
                customize(Popup::menu(&response)).show(add_contents);
                response
            })
        }
    };
}

impl<'a, T> OnResponse<'a, T> {
    #[inline]
    fn into_on_response(self) -> Self
    where
        Self: Sized,
    {
        self
    }

    response_ext_impl!(T, pub);
}

pub trait OnResponseExt<'a>
where
    Self: Sized,
{
    type Target;

    fn into_on_response(self) -> OnResponse<'a, Self::Target>;

    response_ext_impl!(Self::Target,);
}

impl<'a, T> OnResponseExt<'a> for T
where
    T: Widget,
{
    type Target = T;

    #[inline]
    fn into_on_response(self) -> OnResponse<'a, Self::Target>
    where
        Self: Sized,
    {
        OnResponse {
            inner: self,
            on_response: smallvec::SmallVec::new(),
            enabled: true,
        }
    }
}

impl<T: egui::Widget> egui::Widget for OnResponse<'_, T> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let mut response = ui.add_enabled(self.enabled, self.inner);

        for on_response in self.on_response {
            response = on_response(response);
        }

        response
    }
}

impl<T> From<T> for OnResponse<'_, T>
where
    T: egui::Widget,
{
    fn from(inner: T) -> Self {
        Self {
            inner,
            on_response: smallvec::SmallVec::new(),
            enabled: true,
        }
    }
}

impl<T> std::ops::Deref for OnResponse<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> std::ops::DerefMut for OnResponse<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
