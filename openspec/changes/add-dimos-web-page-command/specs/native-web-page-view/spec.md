## MODIFIED Requirements

### Requirement: Blueprint-owned configuration

The system SHALL store Web Page View configuration as blueprint/view state with a required URL and a `show_navigation_controls` setting.

#### Scenario: Blueprint preconfigures a Web Page View

- **WHEN** blueprint state contains a Web Page View with a valid URL and navigation controls setting
- **THEN** the viewer creates the view with those configured values

#### Scenario: Manual creation starts unconfigured

- **WHEN** a user manually adds a Web Page View without setting a URL
- **THEN** the viewer displays a Rerun-side status explaining that no URL is configured

#### Scenario: DimOS command translates to blueprint state

- **WHEN** the DimOS viewer handles an `open_web_page_view` websocket command
- **THEN** the resulting Web Page View URL and navigation-control setting are represented as Web Page View blueprint configuration rather than native-webview-only state
