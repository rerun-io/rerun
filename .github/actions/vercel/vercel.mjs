// @ts-check
import { assert } from "./util.mjs";
/** @import { Deployment, DeploymentBuild, LegacyDeployment, VercelResponse } from "./types" */

/**
 * @typedef {Record<string, string>} Params
 * @typedef {Record<string, string>} Headers
 * @typedef {object} Body
 *
 * @typedef {{ id: string; name: string }} TeamInfo
 * @typedef {{ id: string; name: string }} ProjectInfo
 * @typedef {{ id: string, key: string, value: string }} Env
 *
 * @typedef {"production" | "preview" | "development"} EnvTarget
 * @typedef {"encrypted" | "secret"} EnvType
 */

function isDeploymentReady(
  /** @type {Deployment | DeploymentBuild} */ deployment,
) {
  return deployment.readyState === "READY" || deployment.state === "READY";
}

function isDeploymentFailed(
  /** @type {Deployment | DeploymentBuild} */ deployment,
) {
  if (
    deployment?.readyState?.endsWith("_ERROR") ||
    deployment?.readyState === "ERROR"
  ) {
    return true;
  }
  if (
    (deployment.state && deployment.state.endsWith("_ERROR")) ||
    deployment.state === "ERROR"
  ) {
    return true;
  }
  return false;
}

export function getDeploymentId(
  /** @type {LegacyDeployment | Deployment | DeploymentBuild} */ deployment,
) {
  if ("uid" in deployment) return deployment.uid;
  if ("deploymentId" in deployment && deployment.deploymentId)
    return deployment.deploymentId;
  return deployment.id;
}

export class Project {
  constructor(
    /** @type {Client} */ client,
    /** @type {TeamInfo} */ team,
    /** @type {ProjectInfo} */ project,
  ) {
    this.client = client;
    this.team = team;
    this.project = project;
  }

  /**
   * Return deployments under the current team and project.
   *
   * The endpoint used is a paginated one, but this call does not support pagination,
   * and only returns the first 20 results.
   *
   * The results are sorted by their created date, so the latest deployment
   * for the given `target` is at index `0`.
   * @param {"production" | "preview" | "development"} target
   * @returns {Promise<LegacyDeployment[]>}
   */
  async deployments(target = "production") {
    const response = await this.client.get("v6/deployments", {
      teamId: this.team.id,
      projectId: this.project.id,
      target,
      sort: "created",
    });
    assert(
      "deployments" in response,
      () => `failed to get deployments: ${JSON.stringify(response)}`,
    );
    return response.deployments;
  }

  async latestProductionDeployment() {
    console.log("get latest production deployment");
    const productionDeployments = await this.deployments("production");
    const latestProductionDeployment = productionDeployments[0];
    assert(
      latestProductionDeployment,
      `failed to get latest production deployment`,
    );
    return latestProductionDeployment;
  }

  /**
   * Return environment variables available to the current team and project.
   *
   * @returns {Promise<Env[]>}
   */
  async envs() {
    const response = await this.client.get(
      `v9/projects/${this.project.id}/env`,
      {
        teamId: this.team.id,
      },
    );
    assert(
      "envs" in response,
      () => `failed to get environment variables: ${JSON.stringify(response)}`,
    );
    return response.envs;
  }

  /**
   * Set an environment variable (`envId`), making it available to the current team and project.
   *
   * @param {string} key
   * @param {string} value
   * @param {EnvTarget[]} [target]
   * @param {EnvType} [type]
   * @returns {Promise<any>}
   */
  async setEnv(
    key,
    value,
    target = ["production", "preview", "development"],
    type = "encrypted",
  ) {
    console.log(`set env ${key}=${value} (target: ${target}, type: ${type})`);
    const env = await this.envs().then((envs) =>
      envs.find((env) => env.key === key),
    );
    assert(env);
    return this.client.patch(
      `v9/projects/${this.project.id}/env/${env.id}`,
      { gitBranch: null, key, target, type, value },
      { teamId: this.team.id },
    );
  }

  /**
   * Trigger a redeploy of an existing deployment (`deploymentId`)
   * of a project (`name`) under a specific team (`teamId`).
   *
   * The resulting deployment will be set as the current production deployment.
   *
   * @param {string} deploymentId
   * @param {string} name
   * @returns {Promise<Deployment>}
   */
  async redeploy(deploymentId, name) {
    console.log(`redeploy ${name} (id: ${deploymentId})`);
    return this.client.post(
      `v13/deployments`,
      {
        deploymentId,
        meta: { action: "redeploy" },
        name,
        target: "production",
      },
      { teamId: this.team.id, forceNew: "1" },
    );
  }

  /**
   * Trigger a preview deploy using the files of an existing deployment (`deploymentId`).
   *
   * @param {string} deploymentId
   * @param {string} name
   * @param {Record<string, string>} [env]
   * @returns {Promise<Deployment>}
   */
  async deployPreviewFrom(deploymentId, name, env) {
    console.log(
      `deploy preview from ${name} (id: ${deploymentId}) with env:`,
      env,
    );
    // `target` not being set means "preview"
    const body = {
      deploymentId,
      meta: { action: "redeploy" },
      name,
    };
    if (env) {
      body.env = env;
      body.build = { env };
    }
    return this.client.post(`v13/deployments`, body, {
      teamId: this.team.id,
      forceNew: "1",
    });
  }

  /**
   * @returns {Promise<VercelResponse<Deployment>>}
   */
  async getDeployment(/** @type {string} */ id) {
    return this.client.get(`v13/deployments/${id}`);
  }

  /**
   *
   * @returns {Promise<{ type: "success" | "failure", deployment: Deployment }>}
   */
  async waitForDeployment(/** @type {string} */ id) {
    let deployment = await this.getDeployment(id);
    while (true) {
      if ("error" in deployment) {
        throw new Error(
          `deployment failed: ${JSON.stringify(deployment.error)}`,
        );
      }

      if (isDeploymentFailed(deployment)) {
        return { type: "failure", deployment };
      }

      if (isDeploymentReady(deployment)) {
        return { type: "success", deployment };
      }

      await sleep(1000);
      deployment = await this.getDeployment(getDeploymentId(deployment));
    }
  }
}

async function sleep(/** @type {number} */ ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * @typedef {{
 *   limit: number,
 *   remaining: number,
 *   reset: number,
 * }} RateLimit
 */

/** @returns {RateLimit} */
function parseRateLimit(/** @type {Response} */ response) {
  return {
    limit: parseInt(response.headers.get("X-RateLimit-Limit") || "0"),
    remaining: parseInt(response.headers.get("X-RateLimit-Remaining") || "0"),
    reset: parseInt(response.headers.get("X-RateLimit-Reset") || "0"),
  };
}

function waitFor(/** @type {RateLimit} */ rateLimit) {
  const remainingTime = rateLimit.reset * 1000 + 999 - Date.now();
  console.log(`rate limited by Vercel, retrying after ${remainingTime}ms`);
  return sleep(remainingTime);
}

/**
 * Vercel API Client
 */
export class Client {
  constructor(/** @type {string} */ token) {
    this.token = token;
  }

  /**
   * Helper function to generate the URL for a given `endpoint`
   * with query `params`.
   *
   * @param {string} endpoint
   * @param {Params} [params]
   * @returns {URL}
   */
  url(endpoint, params) {
    const u = new URL(endpoint, "https://api.vercel.com/");
    if (params) for (const key in params) u.searchParams.set(key, params[key]);
    return u;
  }

  /**
   * HTTP GET an `endpoint` with query `params` and `headers`.
   *
   * @template T
   * @param {string} endpoint - not the full URL, just the path, e.g. `v9/projects`
   * @param {Params} [params]
   * @param {Headers} [headers]
   * @returns {Promise<T>}
   */
  async get(endpoint, params, headers) {
    while (true) {
      const url = this.url(endpoint, params);
      const response = await fetch(url, {
        method: "GET",
        headers: {
          Authorization: `Bearer ${this.token}`,
          ...headers,
        },
      });

      const rateLimit = parseRateLimit(response);
      if (rateLimit.remaining < 10) {
        await waitFor(rateLimit);
        continue;
      }

      return response.json();
    }
  }

  /**
   * HTTP POST a `body` to an `endpoint` with query `params` and `headers`.
   *
   * @template T
   * @param {string} endpoint - not the full URL, just the path, e.g. `v9/projects`
   * @param {Body} body - will be JSON encoded
   * @param {Params} [params]
   * @param {Headers} [headers]
   * @returns {Promise<T>}
   */
  async post(endpoint, body, params, headers) {
    const url = this.url(endpoint, params);
    return fetch(url, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${this.token}`,
        "Content-Type": "application/json; charset=utf-8",
        ...headers,
      },
      body: JSON.stringify(body),
    }).then((r) => r.json());
  }

  /**
   * HTTP PATCH a `body` to an `endpoint` with query `params` and `headers`.
   *
   * @template T
   * @param {string} endpoint - not the full URL, just the path, e.g. `v9/projects`
   * @param {Body} body - will be JSON encoded
   * @param {Params} [params]
   * @param {Headers} [headers]
   * @returns {Promise<T>}
   */
  async patch(endpoint, body, params, headers) {
    const url = this.url(endpoint, params);
    return fetch(url, {
      method: "PATCH",
      headers: {
        Authorization: `Bearer ${this.token}`,
        "Content-Type": "application/json; charset=utf-8",
        ...headers,
      },
      body: JSON.stringify(body),
    }).then((r) => r.json());
  }

  /**
   * Return all available teams for the user authorized by this client's token.
   *
   * @returns {Promise<TeamInfo[]>}
   */
  async teams() {
    const response = await this.get("v2/teams");
    assert(
      "teams" in response,
      () => `failed to get teams: ${JSON.stringify(response)}`,
    );
    return response.teams;
  }

  /**
   * Return all available projects under a given team (`teamId`)
   * for the user authorized by this client's token.
   *
   * @param {string} teamId
   * @returns {Promise<ProjectInfo[]>}
   */
  async projects(teamId) {
    const response = await this.get("v9/projects", { teamId });
    assert(
      "projects" in response,
      () => `failed to get projects: ${JSON.stringify(response)}`,
    );
    return response.projects;
  }

  /**
   *
   * @param {string} teamName
   * @param {string} projectName
   */
  async project(teamName, projectName) {
    console.log(`get project ${teamName}/${projectName}`);

    const teams = await this.teams();
    const team = teams.find((team) => team.name === teamName);
    assert(team);
    const projects = await this.projects(team.id);
    const project = projects.find((project) => project.name === projectName);
    assert(project);

    return new Project(this, team, project);
  }
}
