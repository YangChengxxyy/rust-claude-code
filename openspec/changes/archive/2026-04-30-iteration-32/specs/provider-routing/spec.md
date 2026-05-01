## ADDED Requirements

### Requirement: Provider configuration model
The system SHALL support a provider configuration model that can represent Anthropic, Amazon Bedrock, and Google Vertex AI as model request providers. If no provider is explicitly selected, the system SHALL use Anthropic behavior compatible with existing configuration.

#### Scenario: Default provider remains Anthropic
- **WHEN** no CLI, environment, or settings provider override is configured
- **THEN** the system SHALL route model requests through the Anthropic provider

#### Scenario: Settings provider is loaded
- **WHEN** settings or config selects a supported provider
- **THEN** the system SHALL preserve that provider choice in runtime configuration

### Requirement: CLI and environment provider selection
The system SHALL allow provider selection through a `--provider` CLI argument and through provider environment flags. CLI selection SHALL take precedence over environment and settings values.

#### Scenario: CLI provider override
- **WHEN** the CLI is started with `--provider bedrock`
- **THEN** the system SHALL route model requests through the Bedrock provider regardless of lower-priority provider settings

#### Scenario: Bedrock environment flag
- **WHEN** `CLAUDE_CODE_USE_BEDROCK=1` is set and no CLI provider override is present
- **THEN** the system SHALL route model requests through the Bedrock provider

#### Scenario: Vertex environment flag
- **WHEN** `CLAUDE_CODE_USE_VERTEX=1` is set and no CLI provider override is present
- **THEN** the system SHALL route model requests through the Vertex provider

#### Scenario: Conflicting provider environment flags
- **WHEN** both Bedrock and Vertex provider environment flags are enabled and no CLI provider override is present
- **THEN** the system SHALL return a clear configuration error instead of choosing a provider implicitly

### Requirement: Amazon Bedrock request routing
The system SHALL route model requests through Amazon Bedrock when the Bedrock provider is selected. Bedrock requests SHALL use AWS credentials from the runtime environment and provider-specific request signing and endpoint construction.

#### Scenario: Bedrock request is sent
- **WHEN** the Bedrock provider is selected and valid AWS credentials are available
- **THEN** the system SHALL construct a Bedrock Claude endpoint request, sign it with AWS authentication, and return the model response through the existing query loop contract

#### Scenario: Bedrock credentials are missing
- **WHEN** the Bedrock provider is selected but required AWS credentials are unavailable
- **THEN** the system SHALL fail before sending the model request with an actionable authentication error

### Requirement: Google Vertex AI request routing
The system SHALL route model requests through Google Vertex AI when the Vertex provider is selected. Vertex requests SHALL use GCP credentials from the runtime environment and provider-specific endpoint construction.

#### Scenario: Vertex request is sent
- **WHEN** the Vertex provider is selected and valid GCP credentials are available
- **THEN** the system SHALL construct a Vertex Claude endpoint request, authenticate it with Google credentials, and return the model response through the existing query loop contract

#### Scenario: Vertex credentials are missing
- **WHEN** the Vertex provider is selected but required GCP credentials are unavailable
- **THEN** the system SHALL fail before sending the model request with an actionable authentication error

### Requirement: Provider routing preserves agent behavior
Provider selection SHALL NOT change tool execution, permission checks, session message handling, or query loop turn behavior. Provider adapters SHALL normalize provider responses into the stream events consumed by the existing query loop.

#### Scenario: Tools still execute with Bedrock
- **WHEN** the Bedrock provider returns a tool use request
- **THEN** the query loop SHALL execute the requested tool through the existing tool registry and permission system

#### Scenario: Tools still execute with Vertex
- **WHEN** the Vertex provider returns a tool use request
- **THEN** the query loop SHALL execute the requested tool through the existing tool registry and permission system
