# Overview

Our CI workflows make heavy usage of [Reusable Workflows](https://docs.github.com/en/actions/using-workflows/reusing-workflows). These reusable workflows can then be tested manually via the `manual_dispatch.yml` workflow.
Or integrated into CI jobs such has `on_pull_request.yml` or `on_main.yml`.

By convention:

-   All reusable workflows start with the `reusable_` prefix.
-   All workflows that are triggered via `workflow_dispatch` start with the `manual_` prefix.
-   All workflows that are triggered via an event start with the `on_` prefix.
    -   `on_pull_request` is triggered on pull requests.
    -   `on_push_main` is triggered on pushes to the main branch.

If you are going to be doing any editing of workflows, the
[VS Code extension](https://marketplace.visualstudio.com/items?itemName=cschleiden.vscode-github-actions)
for GitHub Actions is highly recommended.
