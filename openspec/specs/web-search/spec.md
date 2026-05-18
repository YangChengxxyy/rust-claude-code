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
The system SHALL support `allowed_domains` and `blocked_domains` filters for search results after provider-specific responses have been normalized.

#### Scenario: Allowed domain filter applies to all providers
- **WHEN** `allowed_domains` is provided for Brave, Tavily, or SearXNG results
- **THEN** the system SHALL only return results from those domains

#### Scenario: Blocked domain filter applies to all providers
- **WHEN** `blocked_domains` is provided for Brave, Tavily, or SearXNG results
- **THEN** the system SHALL exclude results from those domains

### Requirement: WebSearchTool uses configurable backend
The system SHALL use a configurable search backend so that Brave, Tavily, and SearXNG providers can be selected without changing the WebSearch tool input schema.

#### Scenario: Brave remains the default backend
- **WHEN** no new provider is configured and existing Brave configuration is available
- **THEN** WebSearch SHALL use the Brave backend as before

#### Scenario: Tavily backend request
- **WHEN** WebSearch provider is configured as `tavily` and a Tavily API key is available
- **THEN** WebSearch SHALL send the query to Tavily and return normalized title, URL, and summary results

#### Scenario: SearXNG backend request
- **WHEN** WebSearch provider is configured as `searxng` and a SearXNG base URL is available
- **THEN** WebSearch SHALL send the query to the configured SearXNG JSON endpoint and return normalized title, URL, and summary results

#### Scenario: Missing provider credentials
- **WHEN** the configured provider requires credentials or a base URL that is unavailable
- **THEN** WebSearch SHALL return a clear tool error describing the missing configuration

#### Scenario: Configured backend request
- **WHEN** a search backend is configured in settings or tool configuration
- **THEN** the system SHALL send the search query to that backend implementation

#### Scenario: Backend failure
- **WHEN** the configured search backend returns an error
- **THEN** the system SHALL return a tool error result describing the failure
