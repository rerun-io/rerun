## ADDED Requirements

### Requirement: DimOS websocket can request a Web Page View

The system SHALL accept a DimOS websocket command named `open_web_page_view` that requests a native Web Page View panel.

#### Scenario: Command creates a web page panel

- **WHEN** the DimOS viewer receives `open_web_page_view` with `panel_id`, `title`, `url`, and `show_navigation_controls`
- **THEN** the viewer creates a Web Page View configured with that title, URL, and navigation-control preference

### Requirement: Web page commands are idempotent by panel identifier

The system SHALL treat `panel_id` as a caller-owned stable identifier for DimOS web page panel commands.

#### Scenario: Repeated command updates existing panel

- **WHEN** the DimOS viewer receives `open_web_page_view` for a `panel_id` that already has a Web Page View in the current viewer session
- **THEN** the viewer updates that existing panel rather than creating a duplicate panel

#### Scenario: First command creates runtime mapping

- **WHEN** the DimOS viewer receives `open_web_page_view` for a new `panel_id`
- **THEN** the viewer records a runtime-only mapping from that `panel_id` to the created Rerun view identity

### Requirement: Web page commands focus requested panels

The system SHALL focus or raise the requested Web Page View panel when handling an `open_web_page_view` command.

#### Scenario: Existing panel is focused after update

- **WHEN** the DimOS viewer updates an existing Web Page View for an `open_web_page_view` command
- **THEN** the viewer focuses the tab containing that Web Page View when the layout supports tab focus

#### Scenario: Newly created panel is focused

- **WHEN** the DimOS viewer creates a Web Page View for an `open_web_page_view` command
- **THEN** the viewer focuses the newly created panel when the layout supports tab focus

### Requirement: DimOS command scope remains minimal

The system SHALL NOT expose arbitrary viewport tree editing through the `open_web_page_view` command.

#### Scenario: Layout placement fields are ignored or rejected

- **WHEN** an `open_web_page_view` command includes placement fields such as split direction, width ratio, or tab group
- **THEN** the DimOS viewer does not treat those fields as authoritative layout-edit instructions

### Requirement: Invalid web page command URLs are rejected safely

The system SHALL validate URLs from `open_web_page_view` commands using the Web Page View HTTP URL policy before creating or updating panels.

#### Scenario: Unsupported scheme is rejected

- **WHEN** the DimOS viewer receives `open_web_page_view` with a `file://` URL
- **THEN** the viewer rejects the command without creating or updating a Web Page View panel

#### Scenario: HTTP URL is accepted

- **WHEN** the DimOS viewer receives `open_web_page_view` with an `http://` or `https://` URL
- **THEN** the viewer may create or update the requested Web Page View panel
