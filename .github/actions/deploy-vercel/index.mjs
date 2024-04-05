// @ts-check

import { Client } from "./vercel.mjs";
import { assert, getInput, getRequiredInput } from "./util.mjs";
import { deployToProduction } from "./production.mjs";
import { deployToPreview } from "./preview.mjs";

// These inputs are defined in `action.yml`, and should be kept in sync
const token = getRequiredInput("vercel_token");
const team = getRequiredInput("vercel_team_name");
const project = getRequiredInput("vercel_project_name");
const commit = getRequiredInput("release_commit");
const target = getRequiredInput("target");

const client = new Client(token);

switch (target) {
  case "production": {
    const version = getRequiredInput("release_version");
    await deployToProduction(client, { team, project, commit, version });
    break;
  }

  case "preview": {
    await deployToPreview(client, { team, project, commit });
    break;
  }

  default: {
    throw new Error(`"target" must be one of: production, preview`);
  }
}
