// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/text_document.fbs".

#pragma once

#include "../collection.hpp"
#include "../compiler_utils.hpp"
#include "../component_batch.hpp"
#include "../component_descriptor.hpp"
#include "../components/media_type.hpp"
#include "../components/text.hpp"
#include "../indicator_component.hpp"
#include "../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun::archetypes {
    /// **Archetype**: A text element intended to be displayed in its own text box.
    ///
    /// Supports raw text and markdown.
    ///
    /// ## Example
    ///
    /// ### Markdown text document
    /// ![image](https://static.rerun.io/textdocument/babda19558ee32ed8d730495b595aee7a5e2c174/full.png)
    ///
    /// ```cpp
    /// #include <rerun.hpp>
    ///
    /// int main() {
    ///     const auto rec = rerun::RecordingStream("rerun_example_text_document");
    ///     rec.spawn().exit_on_failure();
    ///
    ///     rec.log("text_document", rerun::TextDocument("Hello, TextDocument!"));
    ///
    ///     rec.log(
    ///         "markdown",
    ///         rerun::TextDocument(R"#(# Hello Markdown!
    /// [Click here to see the raw text](recording://markdown:Text).
    ///
    /// Basic formatting:
    ///
    /// | **Feature**       | **Alternative** |
    /// | ----------------- | --------------- |
    /// | Plain             |                 |
    /// | *italics*         | _italics_       |
    /// | **bold**          | __bold__        |
    /// | ~~strikethrough~~ |                 |
    /// | `inline code`     |                 |
    ///
    /// ----------------------------------
    ///
    /// ## Support
    /// - [x] [Commonmark](https://commonmark.org/help/) support
    /// - [x] GitHub-style strikethrough, tables, and checkboxes
    /// - Basic syntax highlighting for:
    ///   - [x] C and C++
    ///   - [x] Python
    ///   - [x] Rust
    ///   - [ ] Other languages
    ///
    /// ## Links
    /// You can link to [an entity](recording://markdown),
    /// a [specific instance of an entity](recording://markdown[#0]),
    /// or a [specific component](recording://markdown:Text).
    ///
    /// Of course you can also have [normal https links](https://github.com/rerun-io/rerun), e.g. <https://rerun.io>.
    ///
    /// ## Image
    /// ![A random image](https://picsum.photos/640/480))#")
    ///             .with_media_type(rerun::MediaType::markdown())
    ///     );
    /// }
    /// ```
    struct TextDocument {
        /// Contents of the text document.
        rerun::components::Text text;

        /// The Media Type of the text.
        ///
        /// For instance:
        /// * `text/plain`
        /// * `text/markdown`
        ///
        /// If omitted, `text/plain` is assumed.
        std::optional<rerun::components::MediaType> media_type;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.components.TextDocumentIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;

      public:
        TextDocument() = default;
        TextDocument(TextDocument&& other) = default;

        explicit TextDocument(rerun::components::Text _text) : text(std::move(_text)) {}

        /// The Media Type of the text.
        ///
        /// For instance:
        /// * `text/plain`
        /// * `text/markdown`
        ///
        /// If omitted, `text/plain` is assumed.
        TextDocument with_media_type(rerun::components::MediaType _media_type) && {
            media_type = std::move(_media_type);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }
    };

} // namespace rerun::archetypes

namespace rerun {
    /// \private
    template <typename T>
    struct AsComponents;

    /// \private
    template <>
    struct AsComponents<archetypes::TextDocument> {
        /// Serialize all set component batches.
        static Result<std::vector<ComponentBatch>> serialize(
            const archetypes::TextDocument& archetype
        );
    };
} // namespace rerun
