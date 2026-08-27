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
