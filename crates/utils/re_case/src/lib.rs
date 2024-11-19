//! Case conversions, the way Rerun likes them.

/// Converts a snake or pascal case input into a snake case output.
///
/// If the input contains multiple parts separated by dots, only the last part is converted.
pub fn to_snake_case(s: &str) -> String {
    use convert_case::{Boundary, Converter, Pattern};

    let rerun_snake = Converter::new()
        .set_boundaries(&[
            Boundary::Hyphen,
            Boundary::Space,
            Boundary::Underscore,
            Boundary::Acronym,
            Boundary::LowerUpper,
        ])
        .set_pattern(Pattern::Lowercase)
        .set_delim("_");

    let mut parts: Vec<_> = s.split('.').map(ToOwned::to_owned).collect();
    if let Some(last) = parts.last_mut() {
        *last = last
            .replace("UVec", "uvec")
            .replace("DVec", "dvec")
            .replace("UInt", "uint");
        *last = rerun_snake.convert(last.as_str());
    }
    parts.join(".")
}

#[test]
fn test_to_snake_case() {
    assert_eq!(
        to_snake_case("rerun.components.Position2D"),
        "rerun.components.position2d"
    );
    assert_eq!(
        to_snake_case("rerun.components.position2d"),
        "rerun.components.position2d"
    );

    assert_eq!(
        to_snake_case("rerun.datatypes.Utf8"),
        "rerun.datatypes.utf8"
    );
    assert_eq!(
        to_snake_case("rerun.datatypes.utf8"),
        "rerun.datatypes.utf8"
    );

    assert_eq!(
        to_snake_case("rerun.datatypes.UVec2D"),
        "rerun.datatypes.uvec2d"
    );
    assert_eq!(
        to_snake_case("rerun.datatypes.uvec2d"),
        "rerun.datatypes.uvec2d"
    );

    assert_eq!(
        to_snake_case("rerun.datatypes.UInt32"),
        "rerun.datatypes.uint32"
    );
    assert_eq!(
        to_snake_case("rerun.datatypes.uint32"),
        "rerun.datatypes.uint32"
    );

    assert_eq!(
        to_snake_case("rerun.archetypes.Points2DIndicator"),
        "rerun.archetypes.points2d_indicator"
    );
    assert_eq!(
        to_snake_case("rerun.archetypes.points2d_indicator"),
        "rerun.archetypes.points2d_indicator"
    );

    assert_eq!(
        to_snake_case("rerun.components.TranslationAndMat3x3"),
        "rerun.components.translation_and_mat3x3"
    );
    assert_eq!(
        to_snake_case("rerun.components.translation_and_mat3x3"),
        "rerun.components.translation_and_mat3x3"
    );

    assert_eq!(
        to_snake_case("rerun.components.AnnotationContext"),
        "rerun.components.annotation_context"
    );
}

/// Converts a snake or pascal case input into a pascal case output.
///
/// If the input contains multiple parts separated by dots, only the last part is converted.
pub fn to_pascal_case(s: &str) -> String {
    use convert_case::{Boundary, Converter, Pattern};

    let rerun_pascal = Converter::new()
        .set_boundaries(&[
            Boundary::Hyphen,
            Boundary::Space,
            Boundary::Underscore,
            Boundary::DigitUpper,
            Boundary::Acronym,
            Boundary::LowerUpper,
        ])
        .set_pattern(Pattern::Capital);

    let mut parts: Vec<_> = s.split('.').map(ToOwned::to_owned).collect();
    if let Some(last) = parts.last_mut() {
        *last = last
            .replace("uvec", "UVec")
            .replace("dvec", "DVec")
            .replace("uint", "UInt")
            .replace("2d", "2D") // NOLINT
            .replace("3d", "3D") // NOLINT
            .replace("4d", "4D");
        *last = rerun_pascal.convert(last.as_str());
    }
    parts.join(".")
}

#[test]
fn test_to_pascal_case() {
    assert_eq!(
        to_pascal_case("rerun.components.position2d"),
        "rerun.components.Position2D"
    );
    assert_eq!(
        to_pascal_case("rerun.components.Position2D"),
        "rerun.components.Position2D"
    );

    assert_eq!(
        to_pascal_case("rerun.datatypes.uvec2d"),
        "rerun.datatypes.UVec2D"
    );
    assert_eq!(
        to_pascal_case("rerun.datatypes.UVec2D"),
        "rerun.datatypes.UVec2D"
    );

    assert_eq!(
        to_pascal_case("rerun.datatypes.uint32"),
        "rerun.datatypes.UInt32"
    );
    assert_eq!(
        to_pascal_case("rerun.datatypes.UInt32"),
        "rerun.datatypes.UInt32"
    );

    assert_eq!(
        to_pascal_case("rerun.archetypes.points2d_indicator"),
        "rerun.archetypes.Points2DIndicator"
    );
    assert_eq!(
        to_pascal_case("rerun.archetypes.Points2DIndicator"),
        "rerun.archetypes.Points2DIndicator"
    );

    assert_eq!(
        to_pascal_case("rerun.components.translation_and_mat3x3"),
        "rerun.components.TranslationAndMat3x3"
    );
    assert_eq!(
        to_pascal_case("rerun.components.TranslationAndMat3x3"),
        "rerun.components.TranslationAndMat3x3"
    );
}

/// Converts a snake or pascal case input into "human case" output, i.e. start with upper case and continue with lower case.
///
/// If the input contains multiple parts separated by dots, only the last part is converted.
pub fn to_human_case(s: &str) -> String {
    use convert_case::{Boundary, Converter, Pattern};

    let rerun_human = Converter::new()
        .set_boundaries(&[
            Boundary::Hyphen,
            Boundary::Space,
            Boundary::Underscore,
            Boundary::LowerDigit,
            Boundary::Acronym,
            Boundary::LowerUpper,
        ])
        .set_pattern(Pattern::Sentence)
        .set_delim(" ");

    let mut parts: Vec<_> = s.split('.').map(ToOwned::to_owned).collect();
    if let Some(last) = parts.last_mut() {
        *last = rerun_human.convert(last.as_str());
        *last = last
            .replace("Uvec", "UVec")
            .replace("Uint", "UInt")
            .replace("U vec", "UVec")
            .replace("U int", "UInt")
            .replace("Int 32", "Int32")
            .replace("mat 3x 3", "mat3x3")
            .replace("mat 4x 4", "mat4x4")
            .replace("2d", "2D") // NOLINT
            .replace("3d", "3D") // NOLINT
            .replace("4d", "4D");
    }
    parts.join(".")
}

#[test]
fn test_to_human_case() {
    assert_eq!(
        to_human_case("rerun.components.position2d"),
        "rerun.components.Position 2D"
    );
    assert_eq!(
        to_human_case("rerun.components.Position2D"),
        "rerun.components.Position 2D"
    );

    assert_eq!(
        to_human_case("rerun.datatypes.uvec2d"),
        "rerun.datatypes.UVec 2D"
    );
    assert_eq!(
        to_human_case("rerun.datatypes.UVec2D"),
        "rerun.datatypes.UVec 2D"
    );

    assert_eq!(
        to_human_case("rerun.datatypes.uint32"),
        "rerun.datatypes.UInt32"
    );
    assert_eq!(
        to_human_case("rerun.datatypes.UInt32"),
        "rerun.datatypes.UInt32"
    );

    assert_eq!(
        to_human_case("rerun.archetypes.points2d_indicator"),
        "rerun.archetypes.Points 2D indicator"
    );
    assert_eq!(
        to_human_case("rerun.archetypes.Points2DIndicator"),
        "rerun.archetypes.Points 2D indicator"
    );

    assert_eq!(
        to_human_case("rerun.components.translation_and_mat3x3"),
        "rerun.components.Translation and mat3x3"
    );
    assert_eq!(
        to_human_case("rerun.components.TranslationAndMat3x3"),
        "rerun.components.Translation and mat3x3"
    );
}
