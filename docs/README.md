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

### Frontmatter

YAML frontmatter at the top of the Markdown file is used for metadata:

```
---
title: Examples
order: 6
---
```

The available attributes are:
| name     | type    | required | description                                   |
| -------- | ------- | -------- | --------------------------------------------- |
| title    | string  | yes      | navigation item title                         |
| order    | number  | yes      | used to sort navigation items                 |
| redirect | string  | no       | redirect to the given url                     |
| hidden   | boolean | no       | don't show the item in navigation             |
| expand   | boolean | no       | expand the sub-items in navigation by default |

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

### Screenshot links

If a screenshot shows an example or snippet which is runnable and built on CI, then you can turn the screenshot
to a link to `rerun.io/viewer` pointing at the example using the `data-inline-viewer` attribute.

Add the attribute to any `<picture>` element like so:

```html
<picture data-inline-viewer="snippets/segmentation_image_simple">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/segmentation_image_simple/eb49e0b8cb870c75a69e2a47a2d202e5353115f6/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/segmentation_image_simple/eb49e0b8cb870c75a69e2a47a2d202e5353115f6/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/segmentation_image_simple/eb49e0b8cb870c75a69e2a47a2d202e5353115f6/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/segmentation_image_simple/eb49e0b8cb870c75a69e2a47a2d202e5353115f6/1200w.png">
  <img src="https://static.rerun.io/segmentation_image_simple/eb49e0b8cb870c75a69e2a47a2d202e5353115f6/full.png">
</picture>
```

The value should be:
- `examples/{NAME}` for examples
- `snippets/{NAME}` for snippets
