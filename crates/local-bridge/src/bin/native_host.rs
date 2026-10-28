//! Chrome Native Messaging uses framed binary stdio, NOT MCP JSON lines.
//! Keep stdout reserved for exactly one framed reply.
fn main() {
    // A detached worker is deliberately the SAME installed executable as
    // the Native Host, without entering its framed stdio protocol.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let [flag, task_id] = args.as_slice() {
        if flag == "--internal-agent-v2-worker" {
            if let Err(error) = local_bridge::agent_task::worker(task_id) {
                eprintln!("Local Bridge Agent worker: {error:#}");
                std::process::exit(1);
            }
            return;
        }
    }
    if let Err(error) = local_bridge::native::serve() {
        eprintln!("Local Bridge native host: {error:#}");
        std::process::exit(1);
    }
}
