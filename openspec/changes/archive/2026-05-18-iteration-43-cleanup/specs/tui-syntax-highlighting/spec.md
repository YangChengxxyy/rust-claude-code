## MODIFIED Requirements

### Requirement: Code blocks support TypeScript and TSX highlighting
The TUI Markdown renderer SHALL apply dedicated syntax highlighting for TypeScript and TSX code fences instead of falling back to plain text or generic JavaScript when a TypeScript-specific syntax is available or embedded.

#### Scenario: TypeScript code fence is highlighted
- **WHEN** a Markdown code block is fenced as `ts` or `typescript`
- **THEN** the renderer SHALL resolve a TypeScript syntax and highlight type annotations distinctly from plain text fallback

#### Scenario: TSX code fence is highlighted
- **WHEN** a Markdown code block is fenced as `tsx` or `typescriptreact`
- **THEN** the renderer SHALL resolve a TSX-capable syntax and highlight JSX markup and TypeScript constructs

#### Scenario: Missing syntax has graceful fallback
- **WHEN** a syntax definition cannot be loaded for a code fence
- **THEN** the renderer SHALL fall back to the existing plain-code rendering without crashing
