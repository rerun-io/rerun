<!--
Open the PR up as a draft until you feel it is ready for a proper review.

Do not make PR:s from your own `main` branch, as that makes it difficult for reviewers to add their own fixes.

Add any improvements to the branch as new commits to make it easier for reviewers to follow the progress. All commits will be squashed to a single commit once the PR is merged into `main`.

Make sure you mention any issues that this PR closes in the description, as well as any other related issues.

To get an auto-generated PR description you can put "copilot:summary" or "copilot:walkthrough" anywhere.
-->

### What

### Checklist
* [ ] I have read and agree to [Contributor Guide](https://github.com/rerun-io/rerun/blob/main/CONTRIBUTING.md) and the [Code of Conduct](https://github.com/rerun-io/rerun/blob/main/CODE_OF_CONDUCT.md)
* [ ] I've included a screenshot or gif (if applicable)
* [ ] I have tested the web demo (if applicable):
  * (PR-local manifest) [app.rerun.io](https://app.rerun.io/pr/{{ pr.number }}/index.html)
  * (nightly manifest) [app.rerun.io](https://app.rerun.io/pr/{{ pr.number }}/index.html?manifest_url=https://app.rerun.io/version/nightly/examples_manifest.json)
* [ ] The PR title and labels are set such as to maximize their usefulness for the next release's CHANGELOG

- [PR Build Summary](https://build.rerun.io/pr/{{ pr.number }})
- [Docs preview](https://rerun.io/preview/{{ pr.commit }}/docs) <!--DOCS-PREVIEW-->
- [Examples preview](https://rerun.io/preview/{{ pr.commit }}/examples) <!--EXAMPLES-PREVIEW-->
- [Recent benchmark results](https://build.rerun.io/graphs/crates.html)
- [Wasm size tracking](https://build.rerun.io/graphs/sizes.html)
