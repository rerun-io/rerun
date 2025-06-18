# C++ docs

A high-level overview of writing and previewing the Rerun C++ documentation.

## Getting started with docs

### Serving the docs locally
Build the docs using:
```
pixi run -e cpp cpp-docs
```
They then can be locally viewed at `rerun_cpp/docs/html/index.html`

### How versioned docs are generated and served
Our online documentation is generated in the same way as above and exists as GCS bucket hosted publicly
on the <https://ref.rerun.io> domain.

Every commit that lands to main will generate bleeding edge documentation to [`docs/cpp/main`](https://ref.rerun.io/docs/cpp/main).
Releases will push to a version instead: [`docs/cpp/0.23.3`](https://ref.rerun.io/docs/cpp/0.23.3)

## Writing docs
Docs are processed by the [`MkDoxy`](https://github.com/JakubAndrysek/MkDoxy) plugin
which internally runs [`Doxygen`](https://www.doxygen.nl/) to extra the docs.

There's many different ways of styling Doxygen compatible comments.
We stick to the following styles:

* use `///` for doc comments
* use `\` for starting [doxygen commands](https://www.doxygen.nl/manual/commands.html)
* whenever possible prefer markdown over [doxygen commands](https://www.doxygen.nl/manual/commands.html)
* Don't use `\brief`, instead write a single line brief description at the top, leave a newline and continue with the detailed description.
* If you want to hide a class or method use `\private`
    * if you have to hide several entries at once, use:
    ```cpp
    /// \cond private
    ...
    /// \endcond
    ```
* Avoid the use of groups when namespaces can be used instead
* Don't omit namespaces when referring to types in docs - instead of `Collection` use `rerun::Collection`.
  Both works usually but the latter makes it easier to understand what is meant.
