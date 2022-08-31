const linearSDK = require('@linear/sdk');
const core = require('@actions/core');
const github = require('@actions/github');


const createLinearIssue = async function(client, title, description, githubUrl) {
    const teams = await client.teams();
    const team = teams.nodes.filter(team => team.name == "Rerun")[0];
  
    const extendedDescription = `
**This issue is a copy of an issue created on Github**
    
- Original issue: ${githubUrl}
- This issue is not automatically kept in sync with the original Github issue.
- Check the original issue for any updates.


**Original description:**
${description}`
  
    const labels = await client.issueLabels();
    const githubLabel = labels.nodes.filter(label => label.name == "Github Issue")[0];
  
    const issue = await client.issueCreate({ teamId: team.id, title: title, description: extendedDescription, labelIds: [githubLabel.id] });

    console.log(`Created linear issue: ${issue}`);
};

try {
    // Get the issue data
    const { issue } = github.context.payload;
    if (!issue) {
        throw new Error("Could not find current issue");
    }
    console.log(`The current issue: ${issue}`);

    // Get the linear client
    const linearAPIToken = core.getInput('linear-api-token');
    const linerClient = new linearSDK.LinearClient({
        apiKey: linearAPIToken
    });

    // Create the new issue
    createLinearIssue(linerClient, issue.title, issue.body, issue.html_url)
  
} catch (error) {
  core.setFailed(error.message);
}