#include <rerun.hpp>

// A very simple custom container type.
template <typename T>
struct CustomContainer {
    T* data;
    size_t size;

    CustomContainer(size_t size_) : data(new T[size_]), size(size_) {}

    // For demonstration purposes: This container can't be copied.
    CustomContainer(const CustomContainer&) = delete;

    ~CustomContainer() {
        delete[] data;
    }
};

// A custom vector type.
struct CustomVectorType {
    float x, y, z;
};

/// Adapts `CustomContainer<CustomVectorType>` to a `ComponentBatch<Position3D>`.
///
/// With this in place, `ComponentBatch<Position3D>` can be constructed from a `CustomContainer<CustomVectorType>`!
template <>
struct rerun::ComponentBatchAdapter<rerun::Position3D, CustomContainer<CustomVectorType>> {
    // Creating a ComponentBatch from a non-temporary is done by casting & borrowing binary compatible data.
    ComponentBatch<rerun::Position3D> operator()(const CustomContainer<CustomVectorType>& container
    ) {
        // Sanity check that this is binary compatible.
        static_assert(sizeof(rerun::Position3D) == sizeof(CustomVectorType));
        static_assert(alignof(rerun::Position3D) <= alignof(CustomVectorType));

        return ComponentBatch<rerun::Position3D>::borrow(
            reinterpret_cast<const rerun::Position3D*>(container.data),
            container.size
        );
    }

    // For temporaries we have to do a copy since the pointer doesn't live long enough.
    // If you don't implement this, the other overload may be used for temporaries and cause
    // undefined behavior.
    ComponentBatch<rerun::Position3D> operator()(CustomContainer<CustomVectorType>&& container) {
        std::vector<rerun::Position3D> components(container.size);
        for (size_t i = 0; i < container.size; ++i) {
            components[i] =
                rerun::Position3D(container.data[i].x, container.data[i].y, container.data[i].z);
        }
        return ComponentBatch<rerun::Position3D>::take_ownership(std::move(components));
    }
};

int main() {
    // Create a new `RecordingStream` which sends data over TCP to the viewer process.
    auto rec = rerun::RecordingStream("rerun_custom_component_adapter");
    rec.spawn().throw_on_failure();

    // Construct some data in a custom format.
    CustomContainer<CustomVectorType> points(3);
    points.data[0] = CustomVectorType{0.0f, 0.0f, 0.0f};
    points.data[1] = CustomVectorType{1.0f, 0.0f, 0.0f};
    points.data[2] = CustomVectorType{0.0f, 1.0f, 0.0f};

    // Log the "my_points" entity with our data, using the `Points3D` archetype.
    // Of course you can mix and match built-in types and custom types on the same archetype.
    rec.log("my_points", rerun::Points3D(points).with_labels({"a", "b", "c"}));
}
