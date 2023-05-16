# Overview

Our CI workflows make heavy usage of [Reusable Workflows](https://docs.github.com/en/actions/using-workflows/reusing-workflows). These reusable workflows can then be tested manually via the `manual_dispatch.yml` workflow.
Or integrated into CI jobs such has `on_pull_request.yml` or `on_main.yml`.

By convention:
- All reusable workflows start with the `reusable_` prefix.
- All workflows that are triggered via `workflow_dispatch` start with the `manual_` prefix.
- All workflows that are triggered via an event start with the `on_` prefix.
  - `on_pull_request` is triggered on pull requests.
  - `on_push_main` is triggered on pushes to the main branch.

If you are going to be doing any editing of workflows, the
[VS Code extension](https://marketplace.visualstudio.com/items?itemName=cschleiden.vscode-github-actions)
for GitHub Actions is highly recommended.

## Reusable Workflows
- [reusable_checks.yml](reusable_checks.yml) - These are all the checks that run to ensure the code is formatted,
  linted, and tested. This job produces no artifacts other than a pass/fail criteria for the build.
  - `SAVE_CACHE` - If true, the rust cache will be saved. Generally we only do this for builds on `main`
- [reusable_bench.yml](reusable_bench.yml) - This job runs the benchmarks to check for performance regressions.
  - `SAVE_BENCH` - If true, then the benchmark results are saved to update https://ref.rerun.io/dev/bench/
- [reusable_deploy_docs](reusable_deploy_docs.yml) - This job deploys the python and rust documentation to https://ref.rerun.io
  - `PY_DOCS_VERSION_NAME` - The name to use for versioning the python docs. This should generally match the version in
    `Cargo.toml`.
  - `UPDATE_LATEST` - If true, then the docs will be deployed to `latest/` as well as the versioned directory.
- [reusable_build_and_test_wheels.yml](reusable_build_and_test_wheels.yml) - This job builds the wheels, runs the
end-to-end test, and produces a sample RRD. The artifacts are accessible via GitHub artifacts, but not otherwise
uploaded anywhere.
  - `MATURIN_FEATURE_FLAGS` - The feature flags to pass to maturin.
  - `PLATFORM` - Which platform to build for: `linux`, `macos-arm`, `macos-intel`, or `windows`.
  - `RELEASE_VERSION` - If producing a release, the version number. This must match the version in `Cargo.toml`.
  - `RRD_ARTIFACT_NAME` - Intermediate name of the GitHub rrd artifact for passing to `reusable_upload_wheels.yml`
  - `SAVE_CACHE` - If true, the rust cache will be saved. Generally we only do this for builds on `main`
  - `WHEEL_ARTIFACT_NAME` - Intermediate name of the GitHub wheel artifact for passing to `reusable_upload_wheels.yml`
- [reusable_upload_wheels.yml](reusable_upload_wheels.yml) - This job uploads the wheels to google cloud
  - `RRD_ARTIFACT_NAME` - Intermediate name of the GitHub rrd artifact. This should match the name passed to
    `reusable_build_and_test_wheels.yml`
  - `WHEEL_ARTIFACT_NAME` - Intermediate name of the GitHub wheel artifact. This should match the name passed to
    `reusable_build_and_test_wheels.yml`
- [reusable_build_web.yml](reusable_build_web.yml) - This job builds the wasm artifacts for the web.
  - `RELEASE_VERSION` - If producing a release, the version number. This must match the version in `Cargo.toml`.
- [reusable_upload_web.yml](reusable_upload_web.yml) - This job uploads the web assets to google cloud. By default this
  only uploads to: `app.rerun.io/commit/<commit>/`
  - `MARK_PRERELEASE_FOR_MAINLINE` - If true, then the web assets will go to `app.rerun.io/prelease/
  - `MARK_TAGGED_VERSION` - If true, then the web assets will go to `app.rerun.io/version/<RELEASE_VERSION>`
  - `RELEASE_VERSION` - If producing a release, the version number.
  - `RRD_ARTIFACT_NAME` - Intermediate name of the GitHub rrd artifact. This should match the name passed to
    `reusable_build_and_test_wheels.yml`
  - `UPLOAD_COMMIT_OVERRIDE` - If set, will replace the value of `<commit>`. This is necessary because we want pull
  request builds associated with their originating commit, even if the web-build happens on an ephemeral merge-commit.
- [reusable_build_web_demo.yml](reusable_build_web.yml) - This job builds the assets uploaded to `demo.rerun.io`.
  - `SOURCE_LINK_COMMIT_OVERRIDE` - If set, will replace the value of `<commit>` in the built app. This ensures that the
  source source code link in the built app always points to the pull request's `HEAD`.
- [reusable_upload_web_demo.yml](reusable_upload_web_demo.yml) - This job uploads the `demo.rerun.io` assets to google cloud. By default this
  only uploads to: `demo.rerun.io/commit/<commit>/`
  - `MARK_PRERELEASE_FOR_MAINLINE` - If true, then the web assets will go to `demo.rerun.io/prelease/
  - `MARK_TAGGED_VERSION` - If true, then the web assets will go to `demo.rerun.io/version/<RELEASE_VERSION>`
  - `RELEASE_VERSION` - If producing a release, the version number.
  - `UPLOAD_COMMIT_OVERRIDE` - If set, will replace the value of `<commit>`. This is necessary because we want pull
  request builds associated with their originating commit, even if the web-build happens on an ephemeral merge-commit.
- [reusable_pr_summary.yml](reusable_pr_summary.yml) - This job updates the PR summary with the results of the CI run.
  - This summary can be found at:
  `https://build.rerun.io/pr/<PR_NUMBER>/`
  - `PR_NUMBER` - The PR number to update. This will generally be set by the `on_pull_request.yml` workflow using:
  `${{github.event.pull_request.number}}`

## Manual Workflows
- [manual_dispatch](manual_dispatch.yml) - This workflow is used to manually trigger the assorted reusable workflows for
  testing.
  - See the workflow file for the list of parameters.
- [manual_build_wheels_for_pr.yml](manual_build_wheels_for_pr.yml) - This workflow can be dispatched on a branch and
  will build all of the wheels for the associated pull-request. Uses:
  - [reusable_build_and_test_wheels.yml](reusable_build_and_test_wheels.yml)
  - [reusable_upload_wheels.yml](reusable_upload_wheels.yml)
  - [reusable_pr_summary.yml](reusable_pr_summary.yml)
