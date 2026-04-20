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
