// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/text_document.fbs".

#pragma once

#include "../collection.hpp"
#include "../compiler_utils.hpp"
#include "../component_batch.hpp"
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
        std::optional<ComponentBatch> text;

        /// The Media Type of the text.
        ///
        /// For instance:
        /// * `text/plain`
        /// * `text/markdown`
        ///
        /// If omitted, `text/plain` is assumed.
        std::optional<ComponentBatch> media_type;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.components.TextDocumentIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        /// The name of the archetype as used in `ComponentDescriptor`s.
        static constexpr const char ArchetypeName[] = "rerun.archetypes.TextDocument";

        /// `ComponentDescriptor` for the `text` field.
        static constexpr auto Descriptor_text = ComponentDescriptor(
            ArchetypeName, "text", Loggable<rerun::components::Text>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `media_type` field.
        static constexpr auto Descriptor_media_type = ComponentDescriptor(
            ArchetypeName, "media_type",
            Loggable<rerun::components::MediaType>::Descriptor.component_name
        );

      public:
        TextDocument() = default;
        TextDocument(TextDocument&& other) = default;
        TextDocument(const TextDocument& other) = default;
        TextDocument& operator=(const TextDocument& other) = default;
        TextDocument& operator=(TextDocument&& other) = default;

        explicit TextDocument(rerun::components::Text _text)
            : text(ComponentBatch::from_loggable(std::move(_text), Descriptor_text).value_or_throw()
              ) {}

        /// Update only some specific fields of a `TextDocument`.
        static TextDocument update_fields() {
            return TextDocument();
        }

        /// Clear all the fields of a `TextDocument`.
        static TextDocument clear_fields();

        /// Contents of the text document.
        TextDocument with_text(const rerun::components::Text& _text) && {
            text = ComponentBatch::from_loggable(_text, Descriptor_text).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// The Media Type of the text.
        ///
        /// For instance:
        /// * `text/plain`
        /// * `text/markdown`
        ///
        /// If omitted, `text/plain` is assumed.
        TextDocument with_media_type(const rerun::components::MediaType& _media_type) && {
            media_type =
                ComponentBatch::from_loggable(_media_type, Descriptor_media_type).value_or_throw();
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
