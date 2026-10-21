// Fails if a TypeScript project uses anything marked `@deprecated`.
//
// `tsc` only reports deprecated usage as an editor suggestion, never as an error,
// so this asks the language service for those suggestions instead.
//
// Usage: node check_deprecated.mjs [path/to/tsconfig.json …]

import path from "node:path";
import ts from "typescript";

let count = 0;
for (const configArg of process.argv.slice(2).length ? process.argv.slice(2) : ["tsconfig.json"]) {
  count += countDeprecatedUses(path.resolve(configArg));
}

/** Print every use of a deprecated API in the project of `configPath`, and return how many. */
function countDeprecatedUses(configPath) {
  const { config, error } = ts.readConfigFile(configPath, ts.sys.readFile);
  if (error) {
    console.error(ts.flattenDiagnosticMessageText(error.messageText, "\n"));
    process.exit(1);
  }
  const { options, fileNames } = ts.parseJsonConfigFileContent(
    config,
    ts.sys,
    path.dirname(configPath),
  );

  const service = ts.createLanguageService({
    getCompilationSettings: () => options,
    getScriptFileNames: () => fileNames,
    getScriptVersion: () => "0",
    getScriptSnapshot: (fileName) => {
      const text = ts.sys.readFile(fileName);
      return text === undefined ? undefined : ts.ScriptSnapshot.fromString(text);
    },
    getCurrentDirectory: () => path.dirname(configPath),
    getDefaultLibFileName: ts.getDefaultLibFilePath,
    fileExists: ts.sys.fileExists,
    readFile: ts.sys.readFile,
    readDirectory: ts.sys.readDirectory,
    directoryExists: ts.sys.directoryExists,
    getDirectories: ts.sys.getDirectories,
  });

  let found = 0;
  for (const fileName of fileNames) {
    for (const diagnostic of service.getSuggestionDiagnostics(fileName)) {
      if (!diagnostic.reportsDeprecated) continue;
      const { line, character } = diagnostic.file.getLineAndCharacterOfPosition(diagnostic.start);
      const message = ts.flattenDiagnosticMessageText(diagnostic.messageText, "\n");
      console.error(`${path.relative(process.cwd(), fileName)}:${line + 1}:${character + 1}: ${message}`);
      found += 1;
    }
  }
  return found;
}

if (count > 0) {
  console.error(`Found ${count} use(s) of deprecated APIs`);
  process.exit(1);
}
console.log("No deprecated APIs used");
