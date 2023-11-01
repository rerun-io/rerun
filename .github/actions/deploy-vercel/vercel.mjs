// @ts-check
import { assert } from "./util.mjs";

/**
 * @typedef {Record<string, string>} Params
 * @typedef {Record<string, string>} Headers
 * @typedef {object} Body
 *
 * @typedef {{ id: string; name: string }} Team
 * @typedef {{ id: string; name: string }} Project
 * @typedef {{ uid: string }} Deployment
 * @typedef {{ id: string, key: string, value: string }} Env
 *
 * @typedef {"production" | "preview" | "development"} EnvTarget
 * @typedef {"encrypted" | "secret"} EnvType
 */

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
    return await fetch(this.url(endpoint, params), {
      headers: {
        Authorization: `Bearer ${this.token}`,
        ...headers,
      },
    }).then((r) => r.json());
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
    return await fetch(this.url(endpoint, params), {
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
    return await fetch(this.url(endpoint, params), {
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
   * @returns {Promise<Team[]>}
   */
  async teams() {
    const response = await this.get("v2/teams");
    assert("teams" in response, () => `failed to get teams: ${JSON.stringify(response)}`);
    return response.teams;
  }

  /**
   * Return all available projects under a given team (`teamId`)
   * for the user authorized by this client's token.
   *
   * @param {string} teamId
   * @returns {Promise<Project[]>}
   */
  async projects(teamId) {
    const response = await this.get("v9/projects", { teamId });
    assert("projects" in response, () => `failed to get projects: ${JSON.stringify(response)}`);
    return response.projects;
  }

  /**
   * Return deployments under a given team (`teamId`) and project (`projectId`).
   *
   * The endpoint used is a paginated one, but this call does not support pagination,
   * and only returns the first 20 results.
   *
   * The results are sorted by their created date, so the latest deployment
   * for the given `target` is at index `0`.
   *
   * @param {string} teamId
   * @param {string} projectId
   * @param {"production" | "preview" | "development"} target
   * @returns {Promise<Deployment[]>}
   */
  async deployments(teamId, projectId, target = "production") {
    const response = await this.get("v6/deployments", {
      teamId,
      projectId,
      target,
      sort: "created",
    });
    assert(
      "deployments" in response,
      () => `failed to get deployments: ${JSON.stringify(response)}`
    );
    return response.deployments;
  }

  /**
   * Return environment variables available to a project (`projectId`) under a team (`teamId`).
   *
   * @param {string} teamId
   * @param {string} projectId
   * @returns {Promise<Env[]>}
   */
  async envs(teamId, projectId) {
    const response = await this.get(`v9/projects/${projectId}/env`, { teamId });
    assert(
      "envs" in response,
      () => `failed to get environment variables: ${JSON.stringify(response)}`
    );
    return response.envs;
  }

  /**
   * Get the decrypted version of an environment variable (`envId`)
   * available to a project (`projectId`) under a team (`teamId`).
   *
   * @param {string} teamId
   * @param {string} projectId
   * @param {string} envId
   * @returns {Promise<Env>}
   */
  async getEnvDecrypted(teamId, projectId, envId) {
    return await this.get(`v9/projects/${projectId}/env/${envId}`, { teamId, decrypt: "true" });
  }

  /**
   * Set an environment variable (`envId`), making it available to a project `projectId`
   * under a team (`teamId`).
   *
   * @param {string} teamId
   * @param {string} projectId
   * @param {string} envId
   * @param {{ key: string, target?: EnvTarget[], type?: EnvType, value: string }} param3
   * @returns {Promise<any>}
   */
  async setEnv(
    teamId,
    projectId,
    envId,
    { key, target = ["production", "preview", "development"], type = "encrypted", value }
  ) {
    return await this.patch(
      `v9/projects/${projectId}/env/${envId}`,
      { gitBranch: null, key, target, type, value },
      { teamId }
    );
  }

  /**
   * Trigger a redeploy of an existing deployment (`deploymentId`)
   * of a project (`name`) under a specific team (`teamId`).
   *
   * The resulting deployment will be set as the current production deployment.
   *
   * @param {string} teamId
   * @param {string} deploymentId
   * @param {string} name
   * @returns {Promise<any>}
   */
  async redeploy(teamId, deploymentId, name) {
    return await this.post(
      `v13/deployments`,
      { deploymentId, meta: { action: "redeploy" }, name, target: "production" },
      { teamId, forceNew: "1" }
    );
  }
}

