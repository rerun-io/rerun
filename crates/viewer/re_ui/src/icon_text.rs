use crate::Icon;
use std::borrow::Cow;

pub enum IconTextItem<'a> {
    Icon(Icon),
    Text(Cow<'a, str>),
}

impl<'a> IconTextItem<'a> {
    pub fn icon(icon: Icon) -> Self {
        Self::Icon(icon)
    }

    pub fn text(text: impl Into<Cow<'a, str>>) -> Self {
        Self::Text(text.into())
    }
}

pub struct IconText<'a>(pub Vec<IconTextItem<'a>>);

impl<'a> IconText<'a> {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn icon(&mut self, icon: Icon) {
        self.0.push(IconTextItem::Icon(icon));
    }

    pub fn text(&mut self, text: impl Into<Cow<'a, str>>) {
        self.0.push(IconTextItem::Text(text.into()));
    }

    pub fn add(&mut self, item: impl Into<IconTextItem<'a>>) {
        self.0.push(item.into());
    }
}

impl<'a> From<Icon> for IconTextItem<'a> {
    fn from(icon: Icon) -> Self {
        IconTextItem::Icon(icon)
    }
}

impl<'a> From<&'a str> for IconTextItem<'a> {
    fn from(text: &'a str) -> Self {
        IconTextItem::Text(text.into())
    }
}

impl<'a> From<String> for IconTextItem<'a> {
    fn from(text: String) -> Self {
        IconTextItem::Text(text.into())
    }
}

#[macro_export]
macro_rules! icon_text {
    ($($item:expr),* $(,)?) => {{
        let mut icon_text = $crate::icon_text::IconText::new();
        $(icon_text.add($item);)*
        icon_text
    }};
}
