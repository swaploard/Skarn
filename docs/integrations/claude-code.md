# Using Skarn with Claude Code / Codex CLI

There are two complementary integrations: the **gateway** (Code Mode + tool
aggregation) and the **shell hook** (sandbox + output compression).

## 1. Gateway as an MCP server

Add Skarn to your MCP config (e.g. `.mcp.json` or the Claude Code settings):

```json
{
  "mcpServers": {
    "skarn": {
      "command": "skarn",
      "args": ["serve"]
    }
  }
}
```

Configure which downstream servers it aggregates in `skarn.toml` (run
`skarn init` for a starter). The agent now sees `search`, `read_tool_docs`, and
`execute` instead of every downstream schema, and orchestrates tools in a
sandboxed isolate.

## 2. Shell hook (sandbox + compression)

Route the agent's shell commands through `skarn run` so they are confined to the
project directory, denied network access, and have their output compressed
70–90% — without changing how the agent prompts.

Run `skarn hook` to print a starter snippet. The essence is to wrap the agent's
command invocation with `skarn run --`:

```jsonc
// .claude/settings.json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [{ "type": "command", "command": "skarn run --net deny --stats --" }]
      }
    ]
  }
}
```
