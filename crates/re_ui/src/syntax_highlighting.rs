use re_entity_db::InstancePath;
use re_log_types::{external::re_types_core::components::InstanceKey, EntityPath, EntityPathPart};

use egui::{text::LayoutJob, Color32, Style, TextFormat};

pub trait SyntaxHighlighting {
    fn syntax_highlighted(&self, style: &Style) -> LayoutJob {
        let mut job = LayoutJob::default();
        self.syntax_highlight_into(style, &mut job);
        job
    }

    fn syntax_highlight_into(&self, style: &Style, job: &mut LayoutJob);
}

fn text_format(style: &Style) -> TextFormat {
    TextFormat {
        font_id: egui::TextStyle::Body.resolve(style),

        // This color be replaced with appropriate color based on widget,
        // and whether the widget is hovered, etc
        color: Color32::PLACEHOLDER,

        ..Default::default()
    }
}

fn faint_text_format(style: &Style) -> TextFormat {
    TextFormat {
        color: Color32::WHITE,

        ..text_format(style)
    }
}

impl SyntaxHighlighting for EntityPathPart {
    fn syntax_highlight_into(&self, style: &Style, job: &mut LayoutJob) {
        job.append(&self.ui_string(), 0.0, text_format(style));
    }
}

impl SyntaxHighlighting for InstanceKey {
    fn syntax_highlight_into(&self, style: &Style, job: &mut LayoutJob) {
        job.append("[", 0.0, faint_text_format(style));
        job.append(&self.to_string(), 0.0, text_format(style));
        job.append("]", 0.0, faint_text_format(style));
    }
}

impl SyntaxHighlighting for EntityPath {
    fn syntax_highlight_into(&self, style: &Style, job: &mut LayoutJob) {
        job.append("/", 0.0, faint_text_format(style));

        for (i, part) in self.iter().enumerate() {
            if i != 0 {
                job.append("/", 0.0, faint_text_format(style));
            }
            part.syntax_highlight_into(style, job);
        }
    }
}

impl SyntaxHighlighting for InstancePath {
    fn syntax_highlight_into(&self, style: &Style, job: &mut LayoutJob) {
        self.entity_path.syntax_highlight_into(style, job);
        if !self.instance_key.is_splat() {
            self.instance_key.syntax_highlight_into(style, job);
        }
    }
}
