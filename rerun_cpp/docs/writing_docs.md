# C++ Docs

A high-level overview of writing and previewing the Rerun C++ documentation.

## Getting started with docs

### Serving the docs locally
Build the docs using:
```
pixi run cpp-docs
```
They then can be locally viewed at `rerun_cpp/docs/html/index.html`

### How versioned docs are generated and served
Our online documentation is generated in the same way and exists as a [GitHub Pages](https://pages.github.com/) project which is hosted from the
contents of the [`gh-pages`](https://github.com/rerun-io/rerun/tree/gh-pages/docs/cpp) branch.

Every commit that lands to main will generate bleeding edge documentation to the [`docs/cpp/main`](https://github.com/rerun-io/rerun/tree/gh-pages/docs/cpp/main).
On a release, when GitHub sees a new tag: `X.Y.Z`, the GitHub action will instead push new docs to `docs/cpp/X.Y.Z`.

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
