---
title: Using Design Tokens
order: 4
---

To help design and code stay in sync, we use an industry standard called Design Tokens. They are simple variables that represent design decisions such as color, typography and shadows.

Our tokens live in a simple object containing all our design decisions:

```
{
  color: {
    red: "#ff0000",
    blue: "#0000ff"
  }
}
```

When building Rerun's UI, we use these tokens by referencing them.

```
backgroundColor: color.red,
color: color.blue
```

The main benefit is that we don't hard code design values in our code. This makes it much easier to evolve our design down the road. When our design evolves, our tokens change and our UI changes with them.

## Global and Alias tokens

There are two types of tokens — Global and Alias tokens. Global tokens are the most basic building blocks:

```
"Global": {
  "Color": {
    "Grey": {
      "0": { "value": "#191c1d" },
      "50": { "value": "#212527" },
      "100": { "value": "#2a2e30" }
      ...
```

We need the Global tokens, but they are problematic. How do you know when to use a specific token? How do we ensure that everone uses the right tokens? That's where Alias tokens come in.

Alias tokens are more specific. They are purposefully designed to be used in specific situations:

```
"Alias": {
  "Color": {
    "Surface": {
      "Default": {
        "description": "Background color for most surfaces",
        "value": "{Global.Color.Grey.0}",
      }
    },
    "Text": {
      "Default": {
        "description": "Default text color",
        "value": "{Global.Color.Grey.800}",
      },
      "Subdued": {
        "description": "Used for less important text",
        "value": "{Global.Color.Grey.650}",
      },
      "Warning": {
        "description": "Text color for warnings and error messages",
        "value": "{Global.Color.Red.300}",
      }
    },
```

With these descriptive Alias tokens, you don't have to wonder which token you should use a specific situation. Printing a warning message? Simply use "Alias.Color.Text.Warning".

Notice that the Alias tokens point back to the Global tokens. This ensures that we all use the same Global tokens behind the scenes.

## Tokens in Figma

We use these tokens in Figma as well. We automatically sync them over. This means that our design files use the same values that we use in code. Everything is in sync. Select any element in Figma to view the tokens that were used.

## Using tokens in code

Our design token API always contain the latest design token values.

```
GET http://www.rerun.io/api/docs/tokens
```

Don't hard code or copy paste these values. Instead, fetch them programatically so that the UI stays up to date when the values in the tokens change. Here's a simple script that fetches the latest tokens and writes them to a local JSON file:

```
import fs from "fs";
import fetch from "node-fetch";

fetch("http://www.rerun.io/api/docs/tokens")
  .then((data) => data.json())
  .then((json) =>
    fs.writeFileSync(
      "tokens.json",
      JSON.stringify(json, null, 2),
      "utf-8"
    )
  );
```
