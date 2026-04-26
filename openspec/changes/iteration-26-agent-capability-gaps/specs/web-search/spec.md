## MODIFIED Requirements

### Requirement: WebSearchTool uses configurable backend
The system SHALL use a configurable real search backend so that different providers can be selected without changing the tool interface. The default dummy backend SHALL NOT be used for normal configured runtime search.

#### Scenario: Configured backend request
- **WHEN** a search backend is configured in settings, environment, or tool configuration
- **THEN** the system SHALL send the search query to that backend implementation

#### Scenario: Missing provider credentials
- **WHEN** the selected search backend requires credentials
- **AND** credentials are not configured
- **THEN** the system SHALL return a clear tool error describing the missing configuration

#### Scenario: Backend failure
- **WHEN** the configured search backend returns an error
- **THEN** the system SHALL return a tool error result describing the failure

#### Scenario: Provider response normalization
- **WHEN** the configured search backend returns provider-specific result fields
- **THEN** the system SHALL normalize them into result title, URL, and summary text before formatting the tool result

## ADDED Requirements

### Requirement: WebSearchTool supports at least one real provider
The system SHALL include at least one real WebSearch provider implementation.

#### Scenario: Real provider returns results
- **WHEN** valid provider configuration and credentials are available
- **AND** the model invokes `WebSearchTool` with a query
- **THEN** the tool SHALL return real search results from the configured provider

#### Scenario: Existing domain filters still apply
- **WHEN** the real provider returns results from multiple domains
- **AND** `allowed_domains` or `blocked_domains` are provided
- **THEN** the system SHALL apply the existing domain filtering rules before formatting the final result

