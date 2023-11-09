#include "translation_and_mat3x3.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace datatypes {

#ifdef 0
        // <CODEGEN_COPY_TO_HEADER>

        /// Identity transformation.
        ///
        /// Applying this transform does not alter an entity's transformation.
        /// It has all optional fields set to `std::nullopt`.
        static const TranslationAndMat3x3 IDENTITY;

        /// Creates a new 3D transform from translation/matrix.
        ///
        /// \param translation_ \copydoc TranslationAndMat3x3::translation
        /// \param mat3x3_ \copydoc TranslationAndMat3x3::mat3x3
        /// \param from_parent_ \copydoc TranslationAndMat3x3::from_parent
        TranslationAndMat3x3(
            const std::optional<Vec3D>& translation_, const std::optional<Mat3x3>& mat3x3_,
            bool from_parent_
        )
            : translation(translation_), mat3x3(mat3x3_), from_parent(from_parent_) {}

        /// From rotation only.
        ///
        /// \param mat3x3_ \copydoc TranslationAndMat3x3::mat3x3
        /// \param from_parent_ \copydoc TranslationAndMat3x3::from_parent
        TranslationAndMat3x3(const Mat3x3& mat3x3_, bool from_parent_ = false)
            : translation(std::nullopt), mat3x3(mat3x3_), from_parent(from_parent_) {}

        /// From translation only.
        ///
        /// \param translation_ \copydoc TranslationAndMat3x3::translation
        /// \param from_parent_ \copydoc TranslationAndMat3x3::from_parent
        TranslationAndMat3x3(const Vec3D& translation_, bool from_parent_ = false)
            : translation(translation_), mat3x3(std::nullopt), from_parent(from_parent_) {}

        // </CODEGEN_COPY_TO_HEADER>

#endif

        const TranslationAndMat3x3 TranslationAndMat3x3::IDENTITY =
            TranslationAndMat3x3(std::nullopt, std::nullopt, false);

    } // namespace datatypes
} // namespace rerun
