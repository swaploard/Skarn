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

