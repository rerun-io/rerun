// @ts-check

import { Client } from "./vercel.mjs";
import { getInput, getRequiredInput, setOutput } from "./util.mjs";
import { deployToProduction } from "./commands/deploy-production.mjs";
import { deployToPreview } from "./commands/deploy-preview.mjs";
import { updateProjectEnv } from "./commands/update-env.mjs";

// All inputs retrieved via `getInput` are defined in `action.yml`, and should be kept in sync

const token = getRequiredInput("vercel_token");
const teamId = getRequiredInput("vercel_team_name");
const projectId = getRequiredInput("vercel_project_name");
const deploymentId = getInput("vercel_deployment_id");
const command = getRequiredInput("command");
const commit = getInput("release_commit");
const version = getInput("release_version");
const target = getInput("target");

const client = new Client(token);

switch (command) {
  case "deploy":
    await deploy(client, teamId, projectId, commit, version);
    break;
  case "wait-for-deployment":
    if (!deploymentId) {
      throw new Error(`"vercel_deployment_id" must be specified`);
    }
    await waitForDeployment(client, teamId, projectId, deploymentId);
    break;
  case "update-env":
    await updateEnv(client, teamId, projectId);
    break;
  default:
    throw new Error(`"command" must be one of: deploy, update-env`);
}

/**
 * @param {Client} client
 * @param {string} teamId
 * @param {string} projectId
 * @param {string | null} commit
 * @param {string | null} version
 */
async function deploy(client, teamId, projectId, commit, version) {
  switch (target) {
    case "production": {
      await deployToProduction(client, {
        team: teamId,
        project: projectId,
        commit,
        version,
      });
      break;
    }

    case "preview": {
      await deployToPreview(client, {
        team: teamId,
        project: projectId,
        commit,
        version,
      });
      break;
    }

    default: {
      throw new Error(`"target" must be one of: production, preview`);
    }
  }
}

/**
 * @param {Client} client
 * @param {string} teamId
 * @param {string} projectId
 * @param {string} deploymentId
 */
async function waitForDeployment(client, teamId, projectId, deploymentId) {
  const project = await client.project(teamId, projectId);

  const result = await project.waitForDeployment(deploymentId);
  setOutput("vercel_preview_result", result.type);
  setOutput("vercel_preview_url", result.deployment.url);
  setOutput("vercel_preview_inspector_url", result.deployment.inspectorUrl);

  return result.deployment;
}

/**
 * @param {Client} client
 * @param {string} teamId
 * @param {string} projectId
 */
async function updateEnv(client, teamId, projectId) {
  if (!commit && !version) {
    throw new Error(
      `one of "release_commit", "release_version" must be specified`,
    );
  }

  await updateProjectEnv(client, {
    team: teamId,
    project: projectId,
    commit,
    version,
  });
}
