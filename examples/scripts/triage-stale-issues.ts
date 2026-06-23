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

const now = Date.now();
const NINETY_DAYS = 90 * 24 * 60 * 60 * 1000;

const stale = (issues.items ?? issues).filter((i: any) => {
  const updated = new Date(i.updated_at).getTime();
  return now - updated > NINETY_DAYS;
});

skarn.log(`scanned ${(issues.items ?? issues).length} issues, ${stale.length} stale`);

if (stale.length > 0) {
  const lines = stale.slice(0, 10).map((i: any) => `#${i.number} ${i.title}`);
  await skarn.server("slack").post_message({
    channel: "#triage",
    text: `${stale.length} stale bugs:\n${lines.join("\n")}`,
  });
}

