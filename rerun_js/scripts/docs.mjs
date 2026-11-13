import path from "node:path";
import fs from "node:fs";
import { scriptDir } from "./common.mjs";

const rootPackageJson = JSON.parse(
  fs.readFileSync(path.resolve(scriptDir, "../package.json"), "utf-8"),
);
const rootDocsDir = path.resolve(scriptDir, "../docs/");

for (const pkg of rootPackageJson.workspaces) {
  const docsDir = path.resolve(scriptDir, `../${pkg}/docs/`);
  const outputDir = path.join(rootDocsDir, pkg);

  console.log(`cp ${docsDir} ${outputDir}`);
  fs.cpSync(docsDir, outputDir, { recursive: true });
}

const mainPackage = "web-viewer";
const indexHtml = `<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <title>Redirecting</title>
  <noscript>
    <meta http-equiv="refresh" content="1; url=${mainPackage}/" />
  </noscript>
  <script>
    window.location.replace("${mainPackage}/" + window.location.hash);
  </script>
</head>
<body>
  Redirecting to <a href="${mainPackage}/">${mainPackage}/</a>…
</body>
</html>`;

fs.writeFileSync(path.join(rootDocsDir, "index.html"), indexHtml, "utf-8");
