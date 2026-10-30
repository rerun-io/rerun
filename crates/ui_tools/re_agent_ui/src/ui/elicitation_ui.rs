use std::collections::BTreeMap;

use re_agent::acp::schema::v1::{
    CreateElicitationResponse, ElicitationAcceptAction, ElicitationAction, ElicitationContentValue,
    ElicitationMode, ElicitationPropertySchema, ElicitationSchema, EnumOption, MultiSelectItems,
};
use re_agent::{AgentSession, ElicitationId};
use re_ui::text_edit::ReTextEdit;
use re_ui::{ButtonExt as _, UiExt as _};

use super::MAX_REQUEST_CONTENT_HEIGHT;

/// Draft answers retained across frames until their session-local question IDs leave the queue.
#[derive(Default)]
pub struct ElicitationUi {
    forms: BTreeMap<ElicitationId, BTreeMap<String, FieldInput>>,
}

/// Editable drafts; text also holds incomplete numeric input and single-select wire values.
enum FieldInput {
    Boolean(bool),
    Choices(Vec<String>),
    Text(String),
}

impl FieldInput {
    fn new(schema: &ElicitationPropertySchema) -> Self {
        match schema {
            ElicitationPropertySchema::String(schema) => {
                Self::Text(schema.default.clone().unwrap_or_default())
            }
            ElicitationPropertySchema::Number(schema) => Self::Text(
                schema
                    .default
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
            ),
            ElicitationPropertySchema::Integer(schema) => Self::Text(
                schema
                    .default
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
            ),
            ElicitationPropertySchema::Boolean(schema) => {
                Self::Boolean(schema.default.unwrap_or_default())
            }
            ElicitationPropertySchema::Array(schema) => {
                Self::Choices(schema.default.clone().unwrap_or_default())
            }
            _ => Self::Text(String::new()),
        }
    }

    /// Validates a draft for submission; blank text and empty selections omit the field.
    fn value(
        &self,
        schema: &ElicitationPropertySchema,
    ) -> Result<Option<ElicitationContentValue>, String> {
        match (self, schema) {
            (Self::Text(text), ElicitationPropertySchema::String(schema)) => {
                // Detect blank answers without normalizing non-empty text sent to the agent.
                if text.trim().is_empty() {
                    return Ok(None);
                }
                if schema.pattern.is_some() || schema.format.is_some() {
                    return Err("This field's validation format is not supported yet. Skip to answer in chat.".to_owned());
                }
                let length = text.chars().count() as u32;
                if schema.min_length.is_some_and(|min| length < min)
                    || schema.max_length.is_some_and(|max| max < length)
                {
                    return Err("Text length is outside the requested limits.".to_owned());
                }
                if schema
                    .enum_values
                    .as_ref()
                    .is_some_and(|options| !options.contains(text))
                    || schema
                        .one_of
                        .as_ref()
                        .is_some_and(|options| !options.iter().any(|option| option.value == *text))
                {
                    return Err("Choose one of the available options.".to_owned());
                }
                Ok(Some(ElicitationContentValue::String(text.clone())))
            }
            (Self::Text(text), ElicitationPropertySchema::Number(schema)) => {
                if text.trim().is_empty() {
                    return Ok(None);
                }
                let value: f64 = text.trim().parse().map_err(|err| {
                    re_log::trace!("Invalid elicitation number: {err}");
                    "Enter a number.".to_owned()
                })?;
                if !value.is_finite()
                    || schema.minimum.is_some_and(|min| value < min)
                    || schema.maximum.is_some_and(|max| max < value)
                {
                    return Err("Number is outside the requested limits.".to_owned());
                }
                Ok(Some(ElicitationContentValue::Number(value)))
            }
            (Self::Text(text), ElicitationPropertySchema::Integer(schema)) => {
                if text.trim().is_empty() {
                    return Ok(None);
                }
                let value: i64 = text.trim().parse().map_err(|err| {
                    re_log::trace!("Invalid elicitation integer: {err}");
                    "Enter a whole number.".to_owned()
                })?;
                if schema.minimum.is_some_and(|min| value < min)
                    || schema.maximum.is_some_and(|max| max < value)
                {
                    return Err("Number is outside the requested limits.".to_owned());
                }
                Ok(Some(ElicitationContentValue::Integer(value)))
            }
            (Self::Boolean(value), ElicitationPropertySchema::Boolean(_)) => {
                Ok(Some(ElicitationContentValue::Boolean(*value)))
            }
            (Self::Choices(values), ElicitationPropertySchema::Array(schema)) => {
                let options = array_options(&schema.items).ok_or_else(|| {
                    "Unsupported selection type. Skip to answer in chat.".to_owned()
                })?;
                if values.is_empty() {
                    return Ok(None);
                }
                let count = values.len() as u64;
                if schema.min_items.is_some_and(|min| count < min)
                    || schema.max_items.is_some_and(|max| max < count)
                    || values
                        .iter()
                        .any(|value| !options.iter().any(|option| option.value == value))
                {
                    return Err("Selection is outside the requested limits.".to_owned());
                }
                Ok(Some(ElicitationContentValue::StringArray(values.clone())))
            }
            _ => Err("Unsupported field type. Skip to answer in chat.".to_owned()),
        }
    }
}

/// Borrows option metadata while keeping the submitted wire value distinct from its display title.
struct Choice<'a> {
    value: &'a str,
    title: &'a str,
    description: Option<&'a str>,
}

impl<'a> Choice<'a> {
    fn untitled(value: &'a str) -> Self {
        Self {
            value,
            title: value,
            description: None,
        }
    }
}

impl<'a> From<&'a EnumOption> for Choice<'a> {
    fn from(option: &'a EnumOption) -> Self {
        Self {
            value: &option.value,
            title: &option.title,
            description: option.description.as_deref(),
        }
    }
}

fn array_options(items: &MultiSelectItems) -> Option<Vec<Choice<'_>>> {
    match items {
        MultiSelectItems::String(items) => Some(
            items
                .values
                .iter()
                .map(|value| Choice::untitled(value))
                .collect(),
        ),
        MultiSelectItems::Titled(items) => Some(items.options.iter().map(Choice::from).collect()),
        _ => None,
    }
}

/// Indent the description below the main choice text.
fn indent_description_text(ui: &mut egui::Ui, description: &str, text_offset: f32) {
    ui.horizontal(|ui| {
        ui.add_space(text_offset);
        ui.add(egui::Label::new(egui::RichText::new(description).weak()).wrap());
    });
}

/// Returns `true` if the user pressed Enter in a text field, to send the form.
fn field_ui(
    ui: &mut egui::Ui,
    name: &str,
    schema: &ElicitationPropertySchema,
    input: &mut FieldInput,
) -> bool {
    let (title, description) = match schema {
        ElicitationPropertySchema::String(schema) => (&schema.title, &schema.description),
        ElicitationPropertySchema::Number(schema) => (&schema.title, &schema.description),
        ElicitationPropertySchema::Integer(schema) => (&schema.title, &schema.description),
        ElicitationPropertySchema::Boolean(schema) => (&schema.title, &schema.description),
        ElicitationPropertySchema::Array(schema) => (&schema.title, &schema.description),
        _ => {
            ui.error_label("Unsupported field type");
            return false;
        }
    };
    let label = title.as_deref().unwrap_or(name);
    ui.label(egui::RichText::new(label).strong());
    if let Some(description) = description {
        ui.markdown_ui(description);
    }
    let mut text_field = None;
    match (input, schema) {
        (FieldInput::Text(text), ElicitationPropertySchema::String(schema)) => {
            let options = schema
                .one_of
                .as_ref()
                .map(|options| options.iter().map(Choice::from).collect::<Vec<_>>())
                .or_else(|| {
                    schema.enum_values.as_ref().map(|options| {
                        options
                            .iter()
                            .map(|value| Choice::untitled(value))
                            .collect()
                    })
                });
            if let Some(options) = options {
                for Choice {
                    value,
                    title,
                    description,
                } in options
                {
                    if ui.radio(text == value, title).clicked() {
                        if text == value {
                            text.clear();
                        } else {
                            *text = value.to_owned();
                        }
                    }
                    if let Some(description) = description {
                        indent_description_text(
                            ui,
                            description,
                            ui.spacing().icon_width + ui.spacing().icon_spacing,
                        );
                    }
                }
            } else {
                text_field = Some(
                    ui.add(
                        ReTextEdit::singleline(text)
                            .hint_text(label)
                            .desired_width(f32::INFINITY),
                    ),
                );
            }
        }
        (FieldInput::Text(text), _) => {
            text_field = Some(
                ui.add(
                    ReTextEdit::singleline(text)
                        .hint_text(label)
                        .desired_width(f32::INFINITY),
                ),
            );
        }
        (FieldInput::Boolean(value), _) => {
            ui.checkbox(value, label);
        }
        (FieldInput::Choices(values), ElicitationPropertySchema::Array(schema)) => {
            if let Some(options) = array_options(&schema.items) {
                for Choice {
                    value,
                    title,
                    description,
                } in options
                {
                    let mut selected = values.iter().any(|selected| selected == value);
                    let response = ui.checkbox(&mut selected, title);
                    if response.changed() {
                        if selected {
                            values.push(value.to_owned());
                        } else {
                            values.retain(|selected| selected != value);
                        }
                    }
                    if let Some(description) = description {
                        let style: egui::widget_style::CheckboxStyle =
                            ui.widget_style(response.id, &Default::default());
                        indent_description_text(
                            ui,
                            description,
                            style.checkbox_size
                                + style.atom_layout.gap
                                + style.atom_layout.frame.total_margin().left,
                        );
                    }
                }
            } else {
                ui.error_label("Unsupported selection type");
            }
        }
        _ => {}
    }
    text_field.is_some_and(|response| {
        response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter))
    })
}

/// Collects validated answers, omitting blank optional fields.
///
/// Nonempty forms require at least one answer even if all schema fields are optional.
fn form_content(
    schema: &ElicitationSchema,
    fields: &BTreeMap<String, FieldInput>,
) -> Result<BTreeMap<String, ElicitationContentValue>, String> {
    let mut content = BTreeMap::new();
    for (name, property) in &schema.properties {
        let input = fields
            .get(name)
            .ok_or_else(|| format!("Missing field: {name}"))?;
        match input.value(property)? {
            Some(value) => {
                content.insert(name.clone(), value);
            }
            None if schema
                .required
                .as_ref()
                .is_some_and(|required| required.contains(name)) =>
            {
                return Err(format!("{name}: an answer is required."));
            }
            None => {}
        }
    }
    if schema
        .required
        .iter()
        .flatten()
        .any(|name| !schema.properties.contains_key(name))
    {
        return Err("The question contains an unsupported required field.".to_owned());
    }
    if !schema.properties.is_empty() && content.is_empty() {
        return Err("Choose an option or enter an answer.".to_owned());
    }
    Ok(content)
}

fn form_ui(
    ui: &mut egui::Ui,
    message: &str,
    schema: &ElicitationSchema,
    fields: &mut BTreeMap<String, FieldInput>,
) -> Option<CreateElicitationResponse> {
    let mut response = None;
    let mut enter_pressed = false;
    egui::Frame::new()
        .fill(ui.tokens().faint_bg_color)
        .corner_radius(6)
        .inner_margin(8)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new("Question from agent").strong());
            egui::ScrollArea::vertical()
                .max_height(MAX_REQUEST_CONTENT_HEIGHT)
                .show(ui, |ui| {
                    ui.markdown_ui(message);
                    for (name, property) in &schema.properties {
                        ui.push_id(name, |ui| {
                            if let Some(input) = fields.get_mut(name) {
                                enter_pressed |= field_ui(ui, name, property, input);
                            }
                        });
                    }
                });
            let content = form_content(schema, fields);
            if let Err(err) = &content {
                ui.weak(err);
            }
            // Bound the row height; horizontal centering uses all allocated vertical space.
            ui.allocate_ui_with_layout(
                // The row holds `size_small` buttons.
                egui::vec2(ui.available_width(), re_ui::Size::Small.height()),
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    let send = ui
                        .add_enabled(
                            content.is_ok(),
                            egui::Button::new("Send").primary().size_small(),
                        )
                        .clicked();
                    if (send || enter_pressed)
                        && let Ok(content) = content
                    {
                        response = Some(CreateElicitationResponse::new(
                            ElicitationAcceptAction::new().content(content),
                        ));
                    }
                    if ui
                        .add(egui::Button::new("Skip").outlined().size_small())
                        .clicked()
                    {
                        response = Some(CreateElicitationResponse::new(ElicitationAction::Decline));
                    }
                },
            );
        });
    response
}

impl ElicitationUi {
    /// Shows pending questions.
    ///
    /// Returns true after submission or Skip so the composer can regain focus.
    pub fn ui(&mut self, ui: &mut egui::Ui, session: &mut AgentSession) -> bool {
        self.forms.retain(|id, _| {
            session
                .pending_elicitations()
                .iter()
                .any(|pending| pending.id == *id)
        });
        let mut answered = None;
        for pending in session.pending_elicitations() {
            let ElicitationMode::Form(form) = &pending.request.mode else {
                continue;
            };
            let fields = self.forms.entry(pending.id).or_insert_with(|| {
                form.requested_schema
                    .properties
                    .iter()
                    .map(|(name, schema)| (name.clone(), FieldInput::new(schema)))
                    .collect()
            });
            let response = ui
                .push_id(("elicitation", pending.id), |ui| {
                    form_ui(ui, &pending.request.message, &form.requested_schema, fields)
                })
                .inner;
            if let Some(response) = response {
                answered = Some((pending.id, response));
            }
        }
        if let Some((id, response)) = answered {
            session.answer_elicitation(id, response);
            self.forms.remove(&id);
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use re_agent::acp::schema::v1::{ElicitationContentValue, ElicitationSchema};

    use super::{FieldInput, form_content};

    #[test]
    fn invalid_answers_cannot_be_submitted() {
        let schema: ElicitationSchema = serde_json::from_value(serde_json::json!({
            "type": "object",
            "properties": {
                "count": {"type": "integer", "minimum": 1, "maximum": 3},
                "score": {"type": "number", "minimum": 0, "maximum": 1},
                "streams": {"type": "array", "items": {"type": "string", "enum": ["camera", "lidar"]}, "maxItems": 1}
            },
            "required": ["count"]
        }))
        .expect("schema");
        let mut fields = schema
            .properties
            .iter()
            .map(|(name, property)| (name.clone(), FieldInput::new(property)))
            .collect();
        assert!(form_content(&schema, &fields).is_err());
        fields.insert("count".to_owned(), FieldInput::Text("4".to_owned()));
        assert!(form_content(&schema, &fields).is_err());
        fields.insert("count".to_owned(), FieldInput::Text("2".to_owned()));
        fields.insert("score".to_owned(), FieldInput::Text("NaN".to_owned()));
        assert!(form_content(&schema, &fields).is_err());
        fields.insert("score".to_owned(), FieldInput::Text("0.5".to_owned()));
        fields.insert(
            "streams".to_owned(),
            FieldInput::Choices(vec!["camera".to_owned(), "lidar".to_owned()]),
        );
        assert!(form_content(&schema, &fields).is_err());
        fields.insert(
            "streams".to_owned(),
            FieldInput::Choices(vec!["camera".to_owned()]),
        );
        let content = form_content(&schema, &fields).expect("valid answers");
        assert_eq!(content["count"], ElicitationContentValue::Integer(2));
        assert_eq!(content["score"], ElicitationContentValue::Number(0.5));
        assert_eq!(
            content["streams"],
            ElicitationContentValue::StringArray(vec!["camera".to_owned()])
        );
    }
}
