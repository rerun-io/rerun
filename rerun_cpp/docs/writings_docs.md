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
TODO: get this part working for C++!


## Writing docs
Docs are processed by the [`MkDoxy`](https://github.com/JakubAndrysek/MkDoxy) plugin
which internally runs [`Doxygen`](https://www.doxygen.nl/) to extra the docs.

There's many different ways of styling Doxygen compatible comments.
We stick to the following styles:

* use `///` for doc comments
* use `\` for starting [doxygen commands](https://www.doxygen.nl/manual/commands.html)
* Don't use `\brief`, instead write a single line brief description at the top, leave a newline and continue with the detailed description.
* TODO: there is definitely more here
