# re_integration_test

This crate contains integration tests for rerun. Some are regular `egui_kittest` integration tests, that wrap the whole
viewer.
Others are based on our `InspectionHarness`, which can run in-process using `egui_kittest` (the default), by connection to a real running
native `rerun-cli` viewer, or to a web viewer running in chrome.

To test with a browser, run:
 - `pixi run rerun-test-web`
 - or manually build the web viewer and run
   `RERUN_INTEGRATION_TEST_TARGET=browser cargo nextest run -p re_integration_test --features browser --profile inspection`
   (--profile) is what instructs nextest to only run the inspection-based tests

To test with native cli, run:
 - `pixi run rerun-build`
 - `RERUN_INTEGRATION_TEST_TARGET=cli cargo nextest run -p re_integration_test --profile inspection`

## Running the browser tests inside a sandbox
The browser tests launch Chrome with `headless_chrome`, which reads the `CHROME` environment variable, then searches `PATH` and the standard install locations.
It silently ignores a `CHROME` path that does not exist.

If Chrome crashes at startup, or the tests hang waiting for it, your shell probably runs inside a sandbox, such as the one of a coding agent.
On macOS, Chrome must register a Mach service at startup, and some sandbox profiles allow that only for Chromium and [Chrome for Testing](https://developer.chrome.com/blog/chrome-for-testing).
Install Chrome for Testing, and set `CHROME` to the executable path that the install command prints:

```sh
npx @puppeteer/browsers install chrome@stable
export CHROME="<printed path>"
test -x "$CHROME"
```

The tests already pass `--no-sandbox` to Chrome, so you do not need to.
