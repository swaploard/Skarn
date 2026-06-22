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

Both read the same `skarn.toml` from the working directory. Run `skarn init` and
uncomment/add the servers you want aggregated. For example:

```toml
[servers.fs]
transport = "stdio"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/path/to/project"]

[servers.postgres]
transport = "stdio"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-postgres", "postgres://localhost/mydb"]
```

The IDE's agent will see the compact `search` / `read_tool_docs` / `execute`
surface. To additionally expose the raw namespaced tools (e.g. `fs__read_file`),
set `passthrough = true` under `[gateway]` or pass `--passthrough` to
`skarn serve`.

## Tip: enable verbose logs

```json
{ "mcpServers": { "skarn": { "command": "skarn", "args": ["serve", "--verbose"] } } }
```

