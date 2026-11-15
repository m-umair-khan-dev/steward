//! Placeholder for the `cairn` command-line tool (DESIGN.md §4.2, Phase 7).

fn main() {
    let arg = std::env::args().nth(1);
    match arg.as_deref() {
        Some("--version") | Some("-V") => println!("cairn-cli {}", env!("CARGO_PKG_VERSION")),
        _ => println!("cairn-cli: no commands yet (app ID {})", cairn_core::APP_ID),
    }
}
