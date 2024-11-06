export interface Deployment {
  id: string;
  deploymentId?: string;
  url: string;
  inspectorUrl: string;
  name: string;
  meta: Dictionary<string | number | boolean>;
  version: 2;
  regions: string[];
  routes: Route[];
  builds?: Builder[];
  functions?: BuilderFunctions;
  plan: string;
  public: boolean;
  ownerId: string;
  readyState:
    | "INITIALIZING"
    | "ANALYZING"
    | "BUILDING"
    | "DEPLOYING"
    | "READY"
    | "QUEUED"
    | "CANCELED"
    | "ERROR";
  state?:
    | "INITIALIZING"
    | "ANALYZING"
    | "BUILDING"
    | "DEPLOYING"
    | "READY"
    | "QUEUED"
    | "CANCELED"
    | "ERROR";
  ready?: number;
  createdAt: number;
  createdIn: string;
  buildingAt?: number;
  creator?: {
    uid?: string;
    email?: string;
    name?: string;
    username?: string;
  };
  env: Dictionary<string>;
  build: {
    env: Dictionary<string>;
  };
  target: string;
  alias: string[];
  aliasAssigned: boolean;
  aliasError: string | null;
  expiration?: number;
  proposedExpiration?: number;
  undeletedAt?: number;
}

export interface DeploymentBuild {
  id: string;
  use: string;
  createdIn: string;
  deployedTo: string;
  readyState:
    | "INITIALIZING"
    | "ANALYZING"
    | "BUILDING"
    | "DEPLOYING"
    | "READY"
    | "ERROR";
  state?:
    | "INITIALIZING"
    | "ANALYZING"
    | "BUILDING"
    | "DEPLOYING"
    | "READY"
    | "ERROR";
  readyStateAt: string;
  path: string;
}

export type LegacyDeployment = {
  aliasAssigned?: number | boolean | null;
  aliasError?: {
    code: string;
    message: string;
  } | null;
  buildingAt: number;
  checksConclusion?: "succeeded" | "failed" | "skipped" | "canceled";
  checksState?: "registered" | "running" | "completed";
  created: number;
  createdAt?: number;
  creator: {
    uid: string;
    email?: string;
    username?: string;
    githubLogin?: string;
    gitlabLogin?: string;
  };
  inspectorUrl: string | null;
  isRollbackCandidate?: boolean | null;
  meta?: { [key: string]: string | undefined };
  name: string;
  ready?: number;
  source?: "cli" | "git" | "import" | "import/repo" | "clone/repo";
  state:
    | "BUILDING"
    | "ERROR"
    | "INITIALIZING"
    | "QUEUED"
    | "READY"
    | "CANCELED";
  target?: "production" | "staging" | null;
  type: "LAMBDAS";
  uid: string;
  url: string;
};

export type VercelApiError = {
  error: {
    code: string;
    message: string;
  };
};

export type VercelResponse<T> = T | VercelApiError;
