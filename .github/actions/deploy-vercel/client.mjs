// @ts-check

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

export class Client {
  constructor(/** @type {string} */ token) {
    this.token = token;
  }

  /**
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
   * @template T
   * @param {string} endpoint
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
   * @template T
   * @param {string} endpoint
   * @param {Body} body
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
   * @template T
   * @param {string} endpoint
   * @param {Body} body
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
   * @returns {Promise<Team[]>}
   */
  async teams() {
    return await this.get("v2/teams").then((r) => r.teams);
  }

  /**
   * @param {string} teamId
   * @returns {Promise<Project[]>}
   */
  async projects(teamId) {
    return await this.get("v9/projects", { teamId }).then((r) => r.projects);
  }

  /**
   * @param {string} teamId
   * @param {string} projectId
   * @param {"production" | "preview" | "development"} target
   * @returns {Promise<Deployment[]>}
   */
  async deployments(teamId, projectId, target = "production") {
    return await this.get("v6/deployments", { teamId, projectId, target, sort: "created" }).then(
      (r) => r.deployments
    );
  }

  /**
   * @param {string} teamId
   * @param {string} projectId
   * @returns {Promise<Env[]>}
   */
  async envs(teamId, projectId) {
    return await this.get(`v9/projects/${projectId}/env`, { teamId }).then((r) => r.envs);
  }

  /**
   * @param {string} teamId
   * @param {string} projectId
   * @param {string} envId
   * @returns {Promise<Env>}
   */
  async getEnvDecrypted(teamId, projectId, envId) {
    return await this.get(`v9/projects/${projectId}/env/${envId}`, { teamId, decrypt: "true" });
  }

  /**
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

