// A self-contained demo that works against the bundled `echo-mcp-server`.
//
//   # from a directory whose skarn.toml has a [servers.echo] entry pointing at
//   # the echo-mcp-server binary:
//   skarn exec --file examples/scripts/echo-demo.js
//
// It chains two tool calls and aggregates locally.

const a = await skarn.callTool("echo", "add", { a: 2, b: 3 });   // { sum: 5 }
const b = await skarn.server("echo").add({ a: a.sum, b: 10 });   // { sum: 15 }
const greeting = await skarn.server("echo").echo({ text: "hello from Code Mode" });

skarn.log("first sum:", a.sum, "second sum:", b.sum);

return {
