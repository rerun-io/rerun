This is the high-level documentation for rerun that is hosted at https://www.rerun.io/docs

## Other documentation
API-level documentation is built from source-code. Python lives at [rerun_py](https://github.com/rerun-io/rerun/tree/main/rerun_py) and Rust in the individual [crates](https://github.com/rerun-io/rerun/tree/main/crates).

## Contributions

Contributions are welcome via pull-request. Note that even landed PRs will not deploy to the main site
until the next time we roll out a site-update. We will generally to do this at least once per release.

## Organization

The site documentation lives in Markdown files inside [`/content`](./content).

The entry point to the documentation is [`/content/index.md`](./content/index.md).

## Special syntax

### Title and Navigation Order
The display titles navigation order of documentation sections are managed by the Metadata at the top of the Markdown
file:
```
---
title: Examples
order: 6
---
```


### Snippets

Snippets are small, self-contained code examples.

To ensure all the code blocks in our documentation contain valid code, we give each one a name, and move it into `snippets/all`:
```
/docs
  /snippets
    /all
      /my-example
        /example.py
```

Each snippet can and should be written in all our supported languages:
```
/docs
  /snippets
    /all
      /my-example
        /example.cpp
        /example.py
        /example.rs
```

Once the snippet is in `snippet/all`, it may be referenced in a documentation Markdown file using this syntax:
```
snippet: my-example
```
