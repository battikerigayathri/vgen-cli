fn main() {
    if let Err(e) = resmate::mcp::run_stdio_server() {
        eprintln!("resmate-mcp error: {}", e);
        std::process::exit(1);
    }
}
