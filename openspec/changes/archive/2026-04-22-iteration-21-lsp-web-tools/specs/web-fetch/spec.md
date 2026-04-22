## ADDED Requirements

### Requirement: WebFetchTool retrieves webpage content
The system SHALL provide a `WebFetchTool` that retrieves webpage content from a given URL and returns a model-friendly text or Markdown result.

#### Scenario: Successful page fetch
- **WHEN** the model invokes `WebFetchTool` with a valid `url`
- **THEN** the system SHALL fetch the page and return extracted content as readable text or Markdown

#### Scenario: Unreachable URL
- **WHEN** the requested URL cannot be reached or returns a fatal network error
- **THEN** the system SHALL return a tool error result describing the failure

### Requirement: WebFetchTool supports content extraction prompts
The system SHALL accept an optional `prompt` input that guides what content should be extracted or summarized from the fetched page.

#### Scenario: Prompt-guided extraction
- **WHEN** the model invokes `WebFetchTool` with `prompt: "Summarize the API authentication section"`
- **THEN** the system SHALL prioritize content relevant to that prompt in the returned result

### Requirement: WebFetchTool truncates large pages
The system SHALL truncate large fetched content to a bounded size suitable for tool results.

#### Scenario: Large page truncation
- **WHEN** the fetched webpage content exceeds the configured size limit
- **THEN** the system SHALL return a truncated result instead of the full page body

### Requirement: WebFetchTool caches recent responses
The system SHALL cache recent successful fetches for a short time-to-live to avoid repeated network requests for the same URL.

#### Scenario: Cache hit within TTL
- **WHEN** the same URL is fetched again within the cache TTL window
- **THEN** the system SHALL return the cached result instead of issuing a new network request
