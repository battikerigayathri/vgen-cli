fn main() {
    if let Err(e) = vgen::mcp::run_stdio_server() {
        eprintln!("vgen-mcp error: {}", e);
        std::process::exit(1);
    }
}
