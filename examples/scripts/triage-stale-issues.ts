// Example Code Mode script for `skarn exec` (or the gateway's `execute` tool).
//
// It demonstrates the core win: fetch a large dataset, filter and summarize it
// *inside the sandbox*, and return only a tiny result — the full issue list
// never enters the model's context window.
//
// Assumes downstream servers named "github" and "slack" in skarn.toml. Adapt the
// tool/argument names to your actual servers (use `search` / `read_tool_docs`).

const issues = await skarn.server("github").search_issues({
  q: "is:open label:bug",
  per_page: 100,
});

