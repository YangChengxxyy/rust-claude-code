use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LspOperation {
    GoToDefinition,
    FindReferences,
    Hover,
    DocumentSymbol,
    WorkspaceSymbol,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspRequest {
    pub method: String,
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspPosition {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspLocation {
    pub uri: String,
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspSymbol {
    pub name: String,
    pub kind: String,
    pub uri: Option<String>,
    pub line: Option<u32>,
    pub character: Option<u32>,
}

impl LspRequest {
    pub fn initialize(root: &Path) -> Self {
        // Ensure root is absolute so the URI gets the triple-slash form file:///…
        let abs = if root.is_absolute() {
            root.to_path_buf()
        } else {
            std::env::current_dir().unwrap_or_default().join(root)
        };
        Self {
            method: "initialize".to_string(),
            params: serde_json::json!({
                "processId": null,
                "rootUri": format!("file://{}", abs.display()),
                "capabilities": {}
            }),
        }
    }

    pub fn go_to_definition(uri: &str, line: u32, character: u32) -> Self {
        Self {
            method: "textDocument/definition".to_string(),
            params: serde_json::json!({
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character }
            }),
        }
    }

    pub fn find_references(uri: &str, line: u32, character: u32) -> Self {
        Self {
            method: "textDocument/references".to_string(),
            params: serde_json::json!({
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character },
                "context": { "includeDeclaration": true }
            }),
        }
    }

    pub fn hover(uri: &str, line: u32, character: u32) -> Self {
        Self {
            method: "textDocument/hover".to_string(),
            params: serde_json::json!({
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character }
            }),
        }
    }

    pub fn document_symbol(uri: &str) -> Self {
        Self {
            method: "textDocument/documentSymbol".to_string(),
            params: serde_json::json!({
                "textDocument": { "uri": uri }
            }),
        }
    }

    pub fn workspace_symbol(query: &str) -> Self {
        Self {
            method: "workspace/symbol".to_string(),
            params: serde_json::json!({ "query": query }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_requests() {
        assert_eq!(LspRequest::go_to_definition("file:///a.rs", 1, 2).method, "textDocument/definition");
        assert_eq!(LspRequest::find_references("file:///a.rs", 1, 2).method, "textDocument/references");
        assert_eq!(LspRequest::hover("file:///a.rs", 1, 2).method, "textDocument/hover");
        assert_eq!(LspRequest::document_symbol("file:///a.rs").method, "textDocument/documentSymbol");
        assert_eq!(LspRequest::workspace_symbol("foo").method, "workspace/symbol");
    }
}
