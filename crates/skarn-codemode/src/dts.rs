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
