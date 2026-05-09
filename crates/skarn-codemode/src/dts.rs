//! Generate a TypeScript declaration file describing the `skarn` API and the
//! available downstream tools, so the LLM can author scripts against real types.

use crate::bridge::ToolDescriptor;

/// Produce a `.d.ts` for the given tool manifest.
///
/// The output documents the global `skarn` object plus a typed `skarn.server()`
/// surface per downstream server, with each tool's description carried through
/// as a JSDoc comment.
pub fn generate_dts(tools: &[ToolDescriptor]) -> String {
    let mut out = String::new();
    out.push_str(DTS_HEADER);

    // Group tools by server, preserving first-seen order.
    let mut servers: Vec<String> = Vec::new();
    for t in tools {
        if !servers.contains(&t.server) {
            servers.push(t.server.clone());
        }
    }

    for server in &servers {
        let iface = server_interface_name(server);
        out.push_str(&format!(
            "\n/** Tools exposed by the `{server}` server. */\n"
        ));
        out.push_str(&format!("interface {iface} {{\n"));
        for t in tools.iter().filter(|t| &t.server == server) {
            if !t.description.is_empty() {
                out.push_str(&format!("  /** {} */\n", t.description.replace('\n', " ")));
            }
            out.push_str(&format!(
                "  {}(args?: Record<string, unknown>): Promise<unknown>;\n",
                js_ident(&t.name)
            ));
        }
        out.push_str("}\n");
    }

    out.push_str("\ninterface SkarnServers {\n");
    for server in &servers {
        out.push_str(&format!(
            "  {}: {};\n",
            json_key(server),
            server_interface_name(server)
        ));
    }
    out.push_str("}\n");

    out.push_str("\ndeclare const skarn: SkarnApi<SkarnServers>;\n");
    out
}

fn server_interface_name(server: &str) -> String {
    let mut s = String::from("Server_");
    for c in server.chars() {
        if c.is_ascii_alphanumeric() {
            s.push(c);
        } else {
            s.push('_');
        }
    }
    s
}

fn js_ident(name: &str) -> String {
    // If the tool name is a valid JS identifier, emit it bare; otherwise quote.
    let valid = !name.is_empty()
        && name.chars().enumerate().all(|(i, c)| {
            c == '_' || c == '$' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit())
        });
    if valid {
        name.to_string()
    } else {
        format!("[\"{}\"]", name.replace('"', "\\\""))
    }
}

fn json_key(name: &str) -> String {
    if js_ident(name) == name {
        name.to_string()
    } else {
        format!("\"{}\"", name.replace('"', "\\\""))
    }
}

const DTS_HEADER: &str = r#"// Skarn Code Mode — ambient type declarations (auto-generated).
//
// Write an async script body. Use `return value;` to return a result to the
// model; only what you return (or `skarn.log`) leaves the sandbox. Intermediate
// data stays local — fetch, filter, and summarize here.

interface SkarnApi<Servers> {
  /** Call a downstream tool. Throws if the tool errors. */
  callTool(server: keyof Servers & string, tool: string, args?: Record<string, unknown>): Promise<unknown>;
  /** Read a resource by URI from a downstream server. */
  readResource(server: keyof Servers & string, uri: string): Promise<unknown>;
  /** List every available tool as `{ server, name, description }[]`. */
  listTools(): Promise<Array<{ server: string; name: string; description: string }>>;
  /** Append a line to the script's log (returned alongside the result). */
  log(...args: unknown[]): void;
  /** Run thunks with bounded concurrency and collect their results in order. */
  parallel<T>(calls: Array<() => Promise<T>>, opts?: { concurrency?: number }): Promise<T[]>;
  /** A per-run key/value scratch space. */
  stash: { put(key: string, value: unknown): void; get(key: string): unknown; keys(): string[] };
  /** A typed proxy for a server's tools: `skarn.server("db").query({...})`. */
  server<K extends keyof Servers & string>(name: K): Servers[K];
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(server: &str, name: &str, desc: &str) -> ToolDescriptor {
        ToolDescriptor {
            server: server.to_string(),
            name: name.to_string(),
            description: desc.to_string(),
            input_schema: serde_json::json!({"type": "object"}),
        }
    }

    #[test]
    fn generates_typed_surface() {
        let tools = vec![
            tool("db", "query", "Run a SQL query"),
            tool("db", "insert", "Insert a row"),
            tool("slack", "post-message", "Post to a channel"),
        ];
        let dts = generate_dts(&tools);
        assert!(dts.contains("interface Server_db"));
        assert!(dts.contains("query(args?:"));
        assert!(dts.contains("Run a SQL query"));
        // A non-identifier tool name must be quoted.
        assert!(dts.contains("[\"post-message\"]"));
