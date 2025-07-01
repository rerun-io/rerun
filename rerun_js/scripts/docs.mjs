import path from "node:path";
import fs from "node:fs";
import { $, script_dir } from "./common.mjs";


const root_package_json = JSON.parse(fs.readFileSync(path.resolve(script_dir, "../package.json"), "utf-8"));
const root_docs_dir = path.resolve(script_dir, "../docs/");

for (const pkg of root_package_json.workspaces) {
  const docs_dir = path.resolve(script_dir, `../${pkg}/docs/`);
  const output_dir = path.join(root_docs_dir, pkg);

  console.log(`cp ${docs_dir} ${output_dir}`);
  fs.cpSync(docs_dir, output_dir, { recursive: true });
}

const main_package = "web-viewer";
const index_html = `<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <title>Redirecting</title>
  <noscript>
    <meta http-equiv="refresh" content="1; url=stable/" />
  </noscript>
  <script>
    window.location.replace("${main_package}/" + window.location.hash);
  </script>
</head>
<body>
  Redirecting to <a href="${main_package}/">${main_package}/</a>...
</body>
</html>`;

fs.writeFileSync(path.join(root_docs_dir, "index.html"), index_html, "utf-8");
