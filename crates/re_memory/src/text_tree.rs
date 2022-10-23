use std::fmt::Write as _;

use crate::*;

pub trait TextTree {
    fn make_text_tree(&self, out: &mut String, depth: usize);

    fn text_tree(&self) -> String {
        let mut out = String::new();
        self.make_text_tree(&mut out, 0);
        out
    }
}

// ----------------------------------------------------------------------------

pub fn format_size(out: &mut String, bytes: usize) {
    if bytes < 1_000 {
        write!(out, "{} B", bytes).unwrap();
    } else if bytes < 1_000_000 {
        write!(out, "{:.2} kB", bytes as f32 / 1_000.0).unwrap();
    } else if bytes < 1_000_000_000 {
        write!(out, "{:.2} MB", bytes as f32 / 1_000_000.0).unwrap();
    } else {
        write!(out, "{:.2} GB", bytes as f32 / 1_000_000_000.0).unwrap();
    }
}

pub fn indent(out: &mut String, depth: usize) {
    const INDENTATION: &str = "                                                                                                ";
    let len = depth * 2;
    let len = len.min(INDENTATION.len());
    out.push_str(&INDENTATION[..len]);
}

// ----------------------------------------------------------------------------

impl TextTree for Summary {
    fn make_text_tree(&self, out: &mut String, _depth: usize) {
        let Self {
            allocated_capacity,
            used,
            shared,
            num_allocs,
        } = *self;

        format_size(out, allocated_capacity);
        if used != allocated_capacity {
            write!(
                out,
                " (used: {:.1}%)",
                100.0 * used as f32 / allocated_capacity as f32
            )
            .unwrap();
        }
        if shared > 0 {
            out.push_str(" + ");
            format_size(out, shared);
            out.push_str(" shared");
        }
        if num_allocs > 0 {
            write!(out, ", {num_allocs} allocations").unwrap();
        }
    }
}

impl TextTree for Map {
    fn make_text_tree(&self, out: &mut String, mut depth: usize) {
        let Self { fields } = self;

        out.push_str("map {\n");
        depth += 1;
        for (name, field) in fields {
            indent(out, depth);
            write!(out, "{name:?}").unwrap();
            out.push_str(": ");
            field.make_text_tree(out, depth);
            out.push('\n');
        }
        depth -= 1;
        indent(out, depth);
        out.push('}');
    }
}

impl TextTree for Struct {
    fn make_text_tree(&self, out: &mut String, mut depth: usize) {
        let Self { type_name, fields } = self;

        out.push_str(type_name);
        out.push_str(" {\n");
        depth += 1;
        for (name, field) in fields {
            indent(out, depth);
            out.push_str(name);
            out.push_str(": ");
            field.make_text_tree(out, depth);
            out.push('\n');
        }
        depth -= 1;
        indent(out, depth);
        out.push('}');
    }
}

impl TextTree for Node {
    fn make_text_tree(&self, out: &mut String, depth: usize) {
        match self {
            Self::Unknown => {
                out.push('?');
            }
            Self::Summary(summary) => summary.make_text_tree(out, depth),
            Self::Map(strct) => strct.make_text_tree(out, depth),
            Self::Struct(strct) => strct.make_text_tree(out, depth),
        }
    }
}

impl TextTree for Global {
    fn make_text_tree(&self, out: &mut String, mut depth: usize) {
        let Self { ref_counted } = self;

        if ref_counted.is_empty() {
            out.push_str("Global { }");
            return;
        }

        out.push_str("Global ref-counted {\n");
        depth += 1;

        for (name, ref_counted) in ref_counted {
            indent(out, depth);
            out.push_str(name);
            out.push_str(":\n");
            depth += 1;
            for instance_info in ref_counted.values() {
                indent(out, depth);
                instance_info.make_text_tree(out, depth);
                out.push('\n');
            }
            depth -= 1;
        }
        depth -= 1;
        indent(out, depth);
        out.push('}');
    }
}

impl TextTree for RefCountedInfo {
    fn make_text_tree(&self, out: &mut String, mut depth: usize) {
        let Self {
            strong_count,
            summary,
        } = self;
        write!(out, "refcount: {strong_count}, ").unwrap();
        depth += 1;
        summary.make_text_tree(out, depth);
    }
}

impl<T> TextTree for TrackingAllocator<T> {
    fn make_text_tree(&self, out: &mut String, mut depth: usize) {
        out.push_str("Allocator {\n");
        depth += 1;

        indent(out, depth);
        format_size(out, self.num_bytes_now());
        writeln!(out, " in {} allocations", self.current_allocations()).unwrap();

        depth -= 1;
        indent(out, depth);
        out.push('}');
    }
}
