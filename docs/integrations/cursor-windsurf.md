# Using Skarn with Cursor / Windsurf

GUI IDEs connect to MCP servers via a JSON config. Point them at Skarn as the
single overarching MCP server to get Code Mode and downstream aggregation.

## Cursor

Edit `~/.cursor/mcp.json` (global) or `.cursor/mcp.json` (per-project):

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

## Windsurf

In Windsurf's MCP settings (`~/.codeium/windsurf/mcp_config.json`):

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

## Configure downstream servers

