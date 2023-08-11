#include "translation_and_mat3x3.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace datatypes {

#ifdef EDIT_EXTENSION
        struct TranslationAndMat3x3Ext {
            std::optional<Vec3D> translation;
            std::optional<Mat3x3> matrix;
            bool from_parent;

#define TranslationAndMat3x3 TranslationAndMat3x3Ext
            // [CODEGEN COPY TO HEADER START]

            static const TranslationAndMat3x3 IDENTITY;

            /// Creates a new 3D transform from translation/matrix.
            ///
            /// @param _from_parent If true, the transform maps from the parent space to the space
            /// where the transform was logged. Otherwise, the transform maps from the space to its
            /// parent.
            TranslationAndMat3x3(
                const std::optional<Vec3D>& _translation, const std::optional<Mat3x3>& _matrix,
                bool _from_parent
            )
                : translation(_translation), matrix(_matrix), from_parent(_from_parent) {}

            /// From rotation only.
            ///
            /// @param _from_parent If true, the transform maps from the parent space to the space
            /// where the transform was logged. Otherwise, the transform maps from the space to its
            /// parent.
            TranslationAndMat3x3(const Mat3x3& _matrix, bool _from_parent = false)
                : translation(std::nullopt), matrix(_matrix), from_parent(_from_parent) {}

            /// From translation only.
            ///
            /// @param _from_parent If true, the transform maps from the parent space to the space
            /// where the transform was logged. Otherwise, the transform maps from the space to its
            /// parent.
            TranslationAndMat3x3(const Vec3D& _translation, bool _from_parent = false)
                : translation(_translation), matrix(std::nullopt), from_parent(_from_parent) {}

            // [CODEGEN COPY TO HEADER END]
        };

#undef TranslationAndMat3x3
#else
#define TranslationAndMat3x3Ext TranslationAndMat3x3
#endif

        const TranslationAndMat3x3Ext TranslationAndMat3x3Ext::IDENTITY =
            TranslationAndMat3x3Ext(std::nullopt, std::nullopt, false);

    } // namespace datatypes
} // namespace rerun
