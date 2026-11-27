---
title: Log Eigen, OpenCV, and custom C++ types without copying
order: 600
description: Log Eigen, OpenCV, or your own C++ types with or without copying
---

In C++, every archetype field takes a [`rerun::Collection`](https://ref.rerun.io/docs/cpp/stable/classrerun_1_1Collection.html).
A collection either _borrows_ your data (no copy) or _owns_ it (a copy in an internal `std::vector`).

## Borrowed vs owned data

- **Borrowed:** the collection only stores a pointer and a length.
  The data must stay alive and unchanged for as long as the collection (or the archetype that holds it) exists.
  When you create the archetype inside the `log` call, this means until `log` returns.
- **Owned:** the data is moved or copied into the collection, so it can outlive your source.

The built-in adapters follow this rule for `std::vector<T>` of a Rerun type:
an lvalue is borrowed, a temporary (or `std::move`d vector) is owned.
Containers of other types are converted with a copy.

```cpp
std::vector<rerun::Position3D> points = make_points();
rec.log("borrowed", rerun::Points3D(points));            // No copy: `points` outlives the call.
rec.log("owned", rerun::Points3D(std::move(points)));    // Moved into the collection.
```

You can also be explicit with `rerun::Collection<T>::borrow(ptr, count)` (or the shorthand `rerun::borrow(ptr, count)`) and `rerun::Collection<T>::take_ownership(std::move(vec))`.

## OpenCV images

Borrow the pixel buffer of a `cv::Mat` directly:

```cpp
cv::Mat img = cv::imread("image.png", cv::IMREAD_COLOR);  // 8-bit BGR

rec.log(
    "image",
    rerun::Image(
        rerun::borrow(img.data, img.total() * img.elemSize()),
        rerun::WidthHeight(static_cast<uint32_t>(img.cols), static_cast<uint32_t>(img.rows)),
        rerun::ColorModel::BGR
    )
);
```

The `cv::Mat` must be continuous in memory (`img.isContinuous()`).
Clone it first if it is a view into a larger image.

## Eigen

`Eigen::Vector3f` has the same memory layout as `rerun::Position3D` (three `float`s), so you can borrow it.
A `3×N` column-major `Eigen::Matrix3Xf` stores its points the same way:

```cpp
Eigen::Matrix3Xf points = …;  // One point per column.

rec.log(
    "points",
    rerun::Points3D(rerun::Collection<rerun::Position3D>::borrow(
        static_cast<const void*>(points.data()),
        static_cast<size_t>(points.cols())
    ))
);
```

The `void*` overload skips the compile-time layout checks, so only use it when you know the layout matches.

For transforms, `rerun::Vec3D` and `rerun::Mat3x3` (column-major, like Eigen) can be built from a pointer:

```cpp
Eigen::Vector3f translation = …;
Eigen::Matrix3f rotation = …;
rec.log("camera", rerun::Transform3D(rerun::Vec3D(translation.data()), rerun::Mat3x3(rotation.data())));
```

## Your own types: `CollectionAdapter`

To log your own container type directly, specialize `rerun::CollectionAdapter<TElement, TContainer>`.
Borrow when the input is an lvalue, and copy when it is a temporary:

```cpp
template <>
struct rerun::CollectionAdapter<rerun::Position3D, MyContainer<MyVec3>> {
    // Non-temporary: borrow binary-compatible data, no copy.
    Collection<rerun::Position3D> operator()(const MyContainer<MyVec3>& container) {
        return Collection<rerun::Position3D>::borrow(container.data, container.size);
    }

    // Temporary: the pointer would dangle, so copy.
    Collection<rerun::Position3D> operator()(MyContainer<MyVec3>&& container) {
        std::vector<rerun::Position3D> components(container.size);
        for (size_t i = 0; i < container.size; ++i) {
            components[i] = rerun::Position3D(container.data[i].x, container.data[i].y, container.data[i].z);
        }
        return Collection<rerun::Position3D>::take_ownership(std::move(components));
    }
};

// Now this works:
rec.log("my_points", rerun::Points3D(my_points));
```

Always implement the `&&` overload.
Without it, a temporary can bind to the `const&` overload, and the collection borrows memory that is freed before it is used.

`Collection::borrow(const T*, …)` checks at compile time that `T` has the same size as the Rerun type and no stricter alignment.

## Examples

- [Custom collection adapter](https://github.com/rerun-io/rerun/tree/latest/examples/cpp/custom_collection_adapter)
- [Eigen and OpenCV](https://github.com/rerun-io/cpp-example-opencv-eigen)
- [🌊 C++ `CollectionAdapter` reference](https://ref.rerun.io/docs/cpp/stable/structrerun_1_1CollectionAdapter.html)
