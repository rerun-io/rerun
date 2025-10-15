//! [`BoxedWidget`] and [`BoxedWidgetExt::boxed`] helpers.
//!
//! [`Box<dyn Widget>`] cannot work because [`Widget`] is not dyn compatible (we can't move out of the
//! Box for the `self` param).
//! Fortunately,  `Box<dyn FnOnce(&mut egui::Ui) -> egui::Response + 'a>` _does_ implement [`Widget`]
//! (Since [`Widget`] is implemented for all `FnOnce(&mut Ui) -> Response`, and `FnOnce` is
//! implemented for all `Box<dyn FnOnce(...)>`).
//!
//! This module contains helpers to box any widgets.
use egui::Widget;

pub type BoxedWidget<'a> = Box<dyn FnOnce(&mut egui::Ui) -> egui::Response + Send + Sync + 'a>;

pub type BoxedWidgetLocal<'a> = Box<dyn FnOnce(&mut egui::Ui) -> egui::Response + 'a>;

pub trait BoxedWidgetExt<'a> {
    fn boxed(self) -> BoxedWidget<'a>;
}

impl<'a, T: 'a> BoxedWidgetExt<'a> for T
where
    T: Widget + Send + Sync,
{
    fn boxed(self) -> BoxedWidget<'a> {
        Box::new(move |ui: &mut egui::Ui| ui.add(self))
    }
}

pub trait BoxedWidgetLocalExt<'a> {
    fn boxed_local(self) -> BoxedWidgetLocal<'a>;
}

impl<'a, T: 'a> BoxedWidgetLocalExt<'a> for T
where
    T: Widget,
{
    fn boxed_local(self) -> BoxedWidgetLocal<'a> {
        Box::new(move |ui: &mut egui::Ui| ui.add(self))
    }
}
