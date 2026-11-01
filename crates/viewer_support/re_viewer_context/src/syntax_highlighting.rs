use egui::Style;
use egui::text::LayoutJob;
use re_entity_db::InstancePath;
use re_log_types::external::re_types_core::{
    ArchetypeName, ComponentDescriptor, ComponentIdentifier, ComponentType,
};
use re_log_types::{ComponentPath, EntityPath, EntityPathPart, Instance};
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;

/// Syntax highlighting for store types, like [`EntityPath`] and [`InstancePath`].
///
/// The orphan rule prevents these types from implementing [`re_ui::SyntaxHighlighting`],
/// since `re_ui` does not depend on the store.
pub trait StoreSyntaxHighlighting {
    fn syntax_highlighted(&self, style: &Style) -> LayoutJob {
        let mut builder = SyntaxHighlightedBuilder::new();
        self.syntax_highlight_into(&mut builder);
        builder.to_job(style)
    }

    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder);
}

impl StoreSyntaxHighlighting for EntityPathPart {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder) {
        builder.append_identifier(&self.ui_string());
    }
}

impl StoreSyntaxHighlighting for Instance {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder) {
        if self.is_all() {
            builder.append_primitive("all");
        } else {
            builder.append_index(&re_format::format_uint(self.get()));
        }
    }
}

impl StoreSyntaxHighlighting for EntityPath {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder) {
        builder.append_syntax("/");

        for (i, part) in self.iter().enumerate() {
            if i != 0 {
                builder.append_syntax("/");
            }
            part.syntax_highlight_into(builder);
        }
    }
}

impl StoreSyntaxHighlighting for InstancePath {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder) {
        self.entity_path.syntax_highlight_into(builder);
        if self.instance.is_specific() {
            InstanceInBrackets(self.instance).syntax_highlight_into(builder);
        }
    }
}

impl StoreSyntaxHighlighting for ComponentType {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder) {
        builder.append_identifier(self.short_name());
    }
}

impl StoreSyntaxHighlighting for ArchetypeName {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder) {
        builder.append_identifier(self.short_name());
    }
}

impl StoreSyntaxHighlighting for ComponentIdentifier {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder) {
        builder.append_identifier(self.as_ref());
    }
}

impl StoreSyntaxHighlighting for ComponentDescriptor {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder) {
        builder.append_identifier(self.display_name());
    }
}

impl StoreSyntaxHighlighting for ComponentPath {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder) {
        let Self {
            entity_path,
            component,
        } = self;
        entity_path.syntax_highlight_into(builder);
        builder.append_syntax(":");
        component.syntax_highlight_into(builder);
    }
}

/// Formats an instance number enclosed in square brackets: `[123]`
pub struct InstanceInBrackets(pub Instance);

impl StoreSyntaxHighlighting for InstanceInBrackets {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder) {
        builder.append_syntax("[");
        self.0.syntax_highlight_into(builder);
        builder.append_syntax("]");
    }
}
