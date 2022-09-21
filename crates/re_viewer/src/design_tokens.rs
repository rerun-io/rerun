pub(crate) fn apply_design_tokens(ctx: &egui::Context) {
    let apply_font = true;
    let apply_font_size = true;

    let design_tokens: serde_json::Value =
        serde_json::from_str(include_str!("../data/design_tokens.json"))
            .expect("Failed to parse data/design_tokens.json");

    let typography_default_path: String =
        get(&design_tokens, &["Alias", "Typography", "Default", "value"]);
    let typography_default: Typography = look_up_or_die(&design_tokens, &typography_default_path);

    if apply_font {
        assert_eq!(typography_default.fontFamily, "Inter");
        assert_eq!(typography_default.fontWeight, "Medium");
        let mut font_definitions = egui::FontDefinitions::default();
        font_definitions.font_data.insert(
            "Inter-Medium".into(),
            egui::FontData::from_static(include_bytes!("../data/Inter-Medium.ttf")),
        );
        font_definitions
            .families
            .get_mut(&egui::FontFamily::Proportional)
            .unwrap()
            .insert(0, "Inter-Medium".into());
        ctx.set_fonts(font_definitions);
    }

    let mut egui_style = egui::Style {
        visuals: egui::Visuals::dark(),
        ..Default::default()
    };

    if apply_font_size {
        let font_size = parse_px(&typography_default.fontSize);

        for text_style in [
            egui::TextStyle::Body,
            egui::TextStyle::Monospace,
            egui::TextStyle::Button,
        ] {
            egui_style.text_styles.get_mut(&text_style).unwrap().size = font_size;
        }
    }

    ctx.set_style(egui_style);
}

fn get<T: serde::de::DeserializeOwned>(mut json: &serde_json::Value, path: &[&str]) -> T {
    for component in path {
        json = json.get(component).unwrap();
    }
    serde_json::from_value(json.clone()).unwrap()
}

#[allow(non_snake_case)]
#[derive(serde::Deserialize)]
struct Typography {
    fontSize: String,
    fontWeight: String,
    fontFamily: String,
    // lineHeight: String,  // TODO(emilk)
    // letterSpacing: String, // TODO(emilk)
}

fn look_up_or_die<T: serde::de::DeserializeOwned>(value: &serde_json::Value, path: &str) -> T {
    let json =
        look_up(value, path).unwrap_or_else(|| panic!("Failed to find {path:?} in design tokens"));
    let json = json.get("value").unwrap();
    serde_json::from_value(json.clone()).unwrap_or_else(|err| {
        panic!(
            "Failed to convert {path:?} to {}: {err}. Json: {json:?}",
            std::any::type_name::<T>()
        )
    })
}

fn look_up<'json>(
    mut value: &'json serde_json::Value,
    path: &str,
) -> Option<&'json serde_json::Value> {
    let path = path.strip_prefix('{')?;
    let path = path.strip_suffix('}')?;
    for component in path.split('.') {
        value = value.get(component)?;
    }
    Some(value)
}

#[test]
fn test_design_tokens() {
    let ctx = egui::Context::default();
    apply_design_tokens(&ctx);

    // Make sure it works:
    let _ = ctx.run(Default::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("Hello Test!");
        });
    });
}

fn parse_px(pixels: &str) -> f32 {
    pixels.strip_suffix("px").unwrap().parse().unwrap()
}
