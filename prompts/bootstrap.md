# Bootstrap Jumble Context

Use this prompt to ask an AI to create jumble context files for a project.

---

## Full Prompt

```
Create jumble context for this project.

Read the AUTHORING.md guide at [JUMBLE_PATH]/AUTHORING.md, then examine this project's structure to create:

1. `.jumble/project.toml` (required)
   - Extract name, description, language from manifest files
   - Identify build/test/lint commands
   - Map 3-5 architectural concepts to their files
   - Note upstream/downstream project relationships

2. `.jumble/conventions.toml`
   - Capture coding patterns to follow (look at existing code)
   - Document gotchas and non-obvious behaviors
   - Check for constitution.md, CONTRIBUTING.md, or similar guides

3. `.jumble/docs.toml`
   - Index the docs/ directory if it exists
   - Write one-line summaries that help find the right doc

Focus on what helps an AI understand this codebase quickly. Don't over-document:
- 3-5 concepts
- 5-7 conventions/gotchas
- Index only human-written docs, not generated API docs
```

---

## Minimal Prompt

```
Create jumble context for this project following the guide at [JUMBLE_PATH]/AUTHORING.md
```

---

## For Workspaces

```
Create jumble context for this workspace.

1. Create `.jumble/workspace.toml` at the workspace root with:
   - Workspace name and description
   - Cross-project conventions (coding standards, tooling)
   - Common gotchas that span multiple projects

2. For each project that needs context, create `.jumble/project.toml` with:
   - Project metadata and commands
   - Key concepts mapped to files
   - Upstream/downstream relationships to other workspace projects

Start with the most important projects. Use `related_projects` to show how they connect.
```

---

## Tips

- Replace `[JUMBLE_PATH]` with the actual path to your jumble installation
- If the AI has access to jumble as an MCP server, it can call `get_prompt("jumble", "bootstrap")` instead
- For large projects, start with project.toml only, then add conventions.toml after reviewing the initial output
