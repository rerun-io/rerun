#include <utility>
#include "annotation_context.hpp"

// <CODEGEN_COPY_TO_HEADER>

#include <type_traits> // std::is_convertible_v

// </CODEGEN_COPY_TO_HEADER>

namespace rerun::components {

#if 0
    // <CODEGEN_COPY_TO_HEADER>

    /// Construct from an initializer list of elements from which `rerun::datatypes::ClassDescriptionMapElem`s can be constructed.
    ///
    /// This will then create a new collection of `rerun::datatypes::ClassDescriptionMapElem`.
    ///
    /// _Implementation note_:
    /// We handle this type of conversion in a generic `rerun::ContainerAdapter`.
    /// However, it is *still* necessary since initializer list overload resolution is handled
    /// in a special way by the compiler, making this case not being covered by the general container case.
    template <
        typename TElement, //
        typename = std::enable_if_t<
            std::is_constructible_v<datatypes::ClassDescriptionMapElem, TElement>> //
        >
    AnnotationContext(std::initializer_list<TElement> class_descriptions) {
        std::vector<datatypes::ClassDescriptionMapElem> class_map_new;
        class_map_new.reserve(class_descriptions.size());
        for (const auto& class_description : class_descriptions) {
            class_map_new.emplace_back(std::move(class_description));
        }
        class_map = Collection<datatypes::ClassDescriptionMapElem>::take_ownership(
            std::move(class_map_new)
        );
    }

    // </CODEGEN_COPY_TO_HEADER>
#endif

} // namespace rerun::components
