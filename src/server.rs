//! MCP Server implementation.

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::config::{
    ProjectConfig,
    ProjectConventions,
    ProjectDocs,
    ProjectPrompts,
    PromptFrontmatter,
    PromptInfo,
    WorkspaceConfig,
};
use crate::protocol::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};
use crate::tools::{self, ProjectData};

/// MCP Server state
pub struct Server {
    pub root: PathBuf,
    pub workspace: Option<WorkspaceConfig>,
    pub projects: HashMap<String, ProjectData>,
}

impl Server {
    pub fn new(root: PathBuf) -> Result<Self> {
        let mut server = Server {
            root,
            workspace: None,
            projects: HashMap::new(),
        };
        server.reload_workspace_and_projects()?;
        Ok(server)
    }

    fn reload_workspace_and_projects(&mut self) -> Result<()> {
        self.workspace = Self::load_workspace_static(&self.root);
        self.projects = self.discover_projects()?;
        Ok(())
    }

    fn load_workspace_static(root: &Path) -> Option<WorkspaceConfig> {
        let workspace_path = root.join(".jumble/workspace.toml");
        if workspace_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&workspace_path) {
                if let Ok(config) = toml::from_str(&content) {
                    return Some(config);
                }
            }
        }
        None
    }

    fn discover_projects(&self) -> Result<HashMap<String, ProjectData>> {
        let mut projects = HashMap::new();
        for entry in WalkDir::new(&self.root)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.ends_with(".jumble/project.toml") {
                if let Ok(config) = self.load_project(path) {
                    let project_dir = path
                        .parent()
                        .and_then(|p| p.parent())
                        .unwrap_or(path)
                        .to_path_buf();

                    // Discover prompts, conventions, and docs
                    let prompts = self.discover_prompts(path.parent().unwrap());
                    let conventions = self.load_conventions(path.parent().unwrap());
                    let docs = self.load_docs(path.parent().unwrap());

                    projects.insert(
                        config.project.name.clone(),
                        (project_dir, config, prompts, conventions, docs),
                    );
                }
            }
        }
        Ok(projects)
    }

    fn discover_prompts(&self, jumble_dir: &Path) -> ProjectPrompts {
        let mut prompts = ProjectPrompts::default();
        let prompts_dir = jumble_dir.join("prompts");

        if prompts_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&prompts_dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.extension().map(|e| e == "md").unwrap_or(false) {
                        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                            let (frontmatter, preview) = match std::fs::read_to_string(&path) {
                                Ok(content) => extract_prompt_frontmatter_and_preview(&content),
                                Err(_) => (None, String::new()),
                            };

                            prompts.prompts.insert(
                                stem.to_string(),
                                PromptInfo {
                                    path: path.clone(),
                                    frontmatter,
                                    preview,
                                },
                            );
                        }
                    }
                }
            }
        }

        prompts
    }

    fn load_conventions(&self, jumble_dir: &Path) -> ProjectConventions {
        let conventions_path = jumble_dir.join("conventions.toml");

        if conventions_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&conventions_path) {
                if let Ok(conventions) = toml::from_str(&content) {
                    return conventions;
                }
            }
        }

        ProjectConventions::default()
    }

    fn load_docs(&self, jumble_dir: &Path) -> ProjectDocs {
        let docs_path = jumble_dir.join("docs.toml");

        if docs_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&docs_path) {
                if let Ok(docs) = toml::from_str(&content) {
                    return docs;
                }
            }
        }

        ProjectDocs::default()
    }

    fn load_project(&self, path: &Path) -> Result<ProjectConfig> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let config: ProjectConfig =
            toml::from_str(&content).with_context(|| format!("Failed to parse {}", path.display()))?;
        Ok(config)
    }

    pub fn handle_request(&mut self, request: JsonRpcRequest) -> JsonRpcResponse {
        let result = match request.method.as_str() {
            "initialize" => self.handle_initialize(&request.params),
            "initialized" => Ok(json!({})),
            "tools/list" => self.handle_tools_list(),
            "tools/call" => self.handle_tools_call(&request.params),
            _ => Err(JsonRpcError {
                code: -32601,
                message: format!("Method not found: {}", request.method),
                data: None,
            }),
        };

        match result {
            Ok(value) => JsonRpcResponse::success(request.id, value),
            Err(error) => JsonRpcResponse::error(request.id, error),
        }
    }

    fn handle_initialize(&self, _params: &Value) -> Result<Value, JsonRpcError> {
        Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "jumble",
                "version": env!("CARGO_PKG_VERSION")
            }
        }))
    }

    fn handle_tools_list(&self) -> Result<Value, JsonRpcError> {
        Ok(tools::tools_list())
    }

    fn handle_tools_call(&mut self, params: &Value) -> Result<Value, JsonRpcError> {
        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsonRpcError {
                code: -32602,
                message: "Missing 'name' parameter".to_string(),
                data: None,
            })?;

        let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

        let result = match name {
            "reload_workspace" => match self.reload_workspace_and_projects() {
                Ok(()) => Ok("Workspace and projects reloaded from disk.".to_string()),
                Err(e) => Err(format!("Failed to reload workspace: {}", e)),
            },
            "list_projects" => tools::list_projects(&self.projects),
            "get_project_info" => tools::get_project_info(&self.projects, &arguments),
            "get_commands" => tools::get_commands(&self.projects, &arguments),
            "get_architecture" => tools::get_architecture(&self.projects, &arguments),
            "get_related_files" => tools::get_related_files(&self.projects, &arguments),
            "list_prompts" => tools::list_prompts(&self.projects, &arguments),
            "get_prompt" => tools::get_prompt(&self.projects, &arguments),
            "get_conventions" => tools::get_conventions(&self.projects, &arguments),
            "get_docs" => tools::get_docs(&self.projects, &arguments),
            "get_workspace_overview" => {
                tools::get_workspace_overview(&self.root, &self.workspace, &self.projects)
            }
            "get_workspace_conventions" => {
                tools::get_workspace_conventions(&self.workspace, &arguments)
            }
            _ => Err(format!("Unknown tool: {}", name)),
        };

        match result {
            Ok(content) => Ok(json!({
                "content": [{
                    "type": "text",
                    "text": content
                }]
            })),
            Err(msg) => Ok(json!({
                "content": [{
                    "type": "text",
                    "text": format!("Error: {}", msg)
                }],
                "isError": true
            })),
        }
    }
}

/// Extract optional YAML frontmatter and a preview snippet from a prompt file.
///
/// Frontmatter is only recognized when the file starts with a line containing only `---`.
/// Everything between the first and second such markers is treated as YAML.
/// The preview is taken from the body that follows the frontmatter (or from the
/// top of the file when no frontmatter is present).
fn extract_prompt_frontmatter_and_preview(
    content: &str,
) -> (Option<PromptFrontmatter>, String) {
    const PREVIEW_MAX_LINES: usize = 16;

    // Helper to build a preview from a body slice.
    fn build_preview(body: &str) -> String {
        body
            .lines()
            .take(PREVIEW_MAX_LINES)
            .collect::<Vec<_>>()
            .join("\n")
    }

    // Detect YAML frontmatter only if the file starts with `---` on the first line.
    if content.starts_with("---\n") {
        // Skip the initial `---\n`.
        let rest = &content[4..];
        if let Some(end_idx) = rest.find("\n---\n") {
            let frontmatter_str = &rest[..end_idx];
            let body_start = end_idx + "\n---\n".len();
            let body = &rest[body_start..];

            let frontmatter = serde_yaml::from_str::<PromptFrontmatter>(frontmatter_str).ok();
            let preview = build_preview(body);
            return (frontmatter, preview);
        }
    }

    // No valid frontmatter header found; fall back to using the first lines of the file.
    (None, build_preview(content))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_frontmatter_and_preview_with_valid_frontmatter() {
        let content = "---\nname: bootstrap\ndescription: Test description\ntags: [a, b]\n---\n# Title\nBody line 1\nBody line 2\n";

        let (frontmatter, preview) = extract_prompt_frontmatter_and_preview(content);

        let fm = frontmatter.expect("expected some frontmatter");
        assert_eq!(fm.name.as_deref(), Some("bootstrap"));
        assert_eq!(fm.description.as_deref(), Some("Test description"));
        assert_eq!(fm.tags, vec!["a", "b"]);

        // Preview should be built from the body after the closing `---`.
        assert!(preview.starts_with("# Title"));
        assert!(preview.contains("Body line 1"));
    }

    #[test]
    fn test_extract_frontmatter_and_preview_without_frontmatter() {
        let content = "# Title\nLine 1\nLine 2\n";

        let (frontmatter, preview) = extract_prompt_frontmatter_and_preview(content);

        assert!(frontmatter.is_none());
        // Preview should include the top of the file when no frontmatter exists.
        assert!(preview.starts_with("# Title"));
        assert!(preview.contains("Line 1"));
    }

    #[test]
    fn test_extract_frontmatter_and_preview_with_unclosed_frontmatter() {
        // Starts with `---` but has no closing marker; this should fall back to no frontmatter.
        let content = "---\nname: broken\n# Title\nLine 1\n";

        let (frontmatter, preview) = extract_prompt_frontmatter_and_preview(content);

        assert!(frontmatter.is_none());
        // In this failure mode we currently treat the whole file as body for the preview.
        assert!(preview.starts_with("---"));
        assert!(preview.contains("name: broken"));
    }
}
