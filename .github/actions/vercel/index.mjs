// @ts-check

import { Client } from "./vercel.mjs";
import { getInput, getRequiredInput } from "./util.mjs";
import { deployToProduction } from "./commands/deploy-production.mjs";
import { deployToPreview } from "./commands/deploy-preview.mjs";
import { updateProjectEnv } from "./commands/update-env.mjs";

// All inputs retrieved via `getInput` are defined in `action.yml`, and should be kept in sync

const token = getRequiredInput("vercel_token");
const team = getRequiredInput("vercel_team_name");
const project = getRequiredInput("vercel_project_name");
const command = getRequiredInput("command");
const commit = getInput("release_commit");
const version = getInput("release_version");
const target = getInput("target");

const client = new Client(token);

switch (command) {
  case "deploy":
    await deploy(client, team, project);
    break;
  case "update-env":
    await updateEnv(client, team, project);
    break;
  default:
    throw new Error(`"command" must be one of: deploy, update-env`);
}

/**
 * @param {Client} client
 * @param {string} team
 * @param {string} project
 */
async function deploy(client, team, project) {
  switch (target) {
    case "production": {
      await deployToProduction(client, { team, project, commit, version });
      break;
    }

    case "preview": {
      await deployToPreview(client, { team, project, commit, version });
      break;
    }

    default: {
      throw new Error(`"target" must be one of: production, preview`);
    }
  }
}

/**
 * @param {Client} client
 * @param {string} team
 * @param {string} project
 */
async function updateEnv(client, team, project) {
  if (!commit && !version) {
    throw new Error(
      `one of "release_commit", "release_version" must be specified`,
    );
  }

  await updateProjectEnv(client, { team, project, commit, version });
}
