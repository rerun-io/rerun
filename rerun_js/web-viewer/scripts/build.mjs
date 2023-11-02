import { $, path, script_dir } from "./common.mjs";

const root_dir = path.resolve(script_dir, "..");

$(`npm run build`, { cwd: root_dir });
