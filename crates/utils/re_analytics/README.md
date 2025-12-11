# Rerun's analytics SDK

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_analytics.svg)](https://crates.io/crates/re_analytics)
[![Documentation](https://docs.rs/re_analytics/badge.svg)](https://docs.rs/re_analytics)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

We use [PostHog](https://posthog.com/) to collect anonymous usage statistics.

Usage data we do collect will be sent to and stored in servers within the EU.

You can audit the actual data being sent out by inspecting the Rerun data directory directly.
Find out its location by running `rerun analytics config`.


### Opting out
Run `rerun analytics disable` to opt out of all usage data collection.

In debug builds, analytics is off by default. Turn it on by setting the environment variable `FORCE_RERUN_ANALYTICS=1`.

### What data is collected?
The exact set of analytics events and parameters can be found here: <https://github.com/rerun-io/rerun/blob/main/crates/utils/re_analytics/src/event.rs>

- We collect high level events about the usage of the Rerun Viewer. For example:
    - The event 'Viewer Opened' helps us estimate how often Rerun is used.
    - The event 'Data Source Connected' helps us understand if users tend to use live
    data sources or recordings most, which helps us prioritize features.
- We associate events with:
    - Metadata about the Rerun build (version, target platform, etc).
    - A persistent random id that is used to associate events from
        multiple sessions together. To regenerate it run `rerun analytics clear`.
- We may associate these events with a hashed `application_id` and `recording_id`,
    so that we can understand if users are more likely to look at few applications often,
    or tend to use Rerun for many temporary scripts. Again, this helps us prioritize.
- We may for instance add events that help us understand how well the auto-layout works.
- If you log in to Rerun's cloud services in the app, the login event is stored with your
    email address and organization.

### What data is NOT collected?
- No Personally Identifiable Information, such as user name or IP address, is collected if
    you don't log in to our cloud services and use Rerun anonymously.
    - This assumes you don't manually and explicitly associate your email with
    the analytics events using the analytics helper cli.
    (Don't do this, it's just meant for internal use for the Rerun team.)
- No user data logged to Rerun is collected.
    - In some cases we collect secure hashes of user provided names (e.g. `application_id`),
    but take great care do this only when we have a clear understanding of why it's needed
    and it won't risk leaking anything potentially proprietary.

### Why do we collect data?
- To improve the Rerun open source library.
