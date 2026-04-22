## ADDED Requirements

### Requirement: WebSearchTool returns structured search results
The system SHALL provide a `WebSearchTool` that issues a search query through a configured search backend and returns a formatted list of results.

#### Scenario: Successful search
- **WHEN** the model invokes `WebSearchTool` with a `query`
- **THEN** the system SHALL return a list of results containing title, URL, and summary text

#### Scenario: No results
- **WHEN** the search backend returns no matches
- **THEN** the system SHALL return an empty-results message instead of failing

### Requirement: WebSearchTool supports domain filtering
The system SHALL support `allowed_domains` and `blocked_domains` filters for search results.

#### Scenario: Allowed domain filter
- **WHEN** `allowed_domains` is provided
- **THEN** the system SHALL only return results from those domains

#### Scenario: Blocked domain filter
- **WHEN** `blocked_domains` is provided
- **THEN** the system SHALL exclude results from those domains

### Requirement: WebSearchTool uses configurable backend
The system SHALL use a configurable search backend so that different providers can be selected without changing the tool interface.

#### Scenario: Configured backend request
- **WHEN** a search backend is configured in settings or tool configuration
- **THEN** the system SHALL send the search query to that backend implementation

#### Scenario: Backend failure
- **WHEN** the configured search backend returns an error
- **THEN** the system SHALL return a tool error result describing the failure
