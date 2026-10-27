//! Chrome Native Messaging uses framed binary stdio, NOT MCP JSON lines.
//! Keep stdout reserved for exactly one framed reply.
fn main() {
    if let Err(error) = local_bridge::native::serve() {
        eprintln!("Local Bridge native host: {error:#}");
        std::process::exit(1);
    }
}
