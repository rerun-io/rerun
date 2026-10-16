//! Human-readable transcript summaries for structured user input.

use agent_client_protocol::schema::v1::{
    CreateElicitationRequest, CreateElicitationResponse, ElicitationAction,
    ElicitationContentValue, ElicitationMode, ElicitationPropertySchema,
};

/// A transcript note using the agent's field titles and single-select labels when supplied.
pub fn response_note(
    request: &CreateElicitationRequest,
    response: &CreateElicitationResponse,
) -> String {
    let answer = match &response.action {
        ElicitationAction::Accept(answer) => answer
            .content
            .as_ref()
            .map(|content| {
                content
                    .iter()
                    .map(|(key, value)| {
                        let property = match &request.mode {
                            ElicitationMode::Form(form) => {
                                form.requested_schema.properties.get(key)
                            }
                            _ => None,
                        };
                        let title = property.and_then(property_title).unwrap_or(key);
                        let value = answer_text(value, property);
                        format!("{title}: {value}")
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default(),
        ElicitationAction::Decline => "Skipped".to_owned(),
        _ => "Cancelled".to_owned(),
    };
    let message = &request.message;
    format!("{message}\n{answer}")
}

fn property_title(property: &ElicitationPropertySchema) -> Option<&str> {
    match property {
        ElicitationPropertySchema::Array(schema) => schema.title.as_deref(),
        ElicitationPropertySchema::Boolean(schema) => schema.title.as_deref(),
        ElicitationPropertySchema::Integer(schema) => schema.title.as_deref(),
        ElicitationPropertySchema::Number(schema) => schema.title.as_deref(),
        ElicitationPropertySchema::String(schema) => schema.title.as_deref(),
        _ => None,
    }
}

fn answer_text(
    value: &ElicitationContentValue,
    property: Option<&ElicitationPropertySchema>,
) -> String {
    match value {
        ElicitationContentValue::String(value) => match property {
            Some(ElicitationPropertySchema::String(schema)) => schema
                .one_of
                .as_ref()
                .and_then(|options| options.iter().find(|option| option.value == *value))
                .map_or_else(|| value.clone(), |option| option.title.clone()),
            _ => value.clone(),
        },
        ElicitationContentValue::StringArray(values) => values.join(", "),
        _ => serde_json::to_string(value).unwrap_or_else(|err| {
            re_log::debug!("Failed to format elicitation answer: {err}");
            String::new()
        }),
    }
}
