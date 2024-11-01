import * as Vercel from "./vercel.mjs";

const client = new Vercel.Client(process.env.VERCEL_TOKEN);

const project = await client.project("rerun", "landing");

const deployment = await project.getDeployment(process.env.DEPLOYMENT_ID);

console.log(deployment);
