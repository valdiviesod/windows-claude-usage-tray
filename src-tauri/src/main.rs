#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--statusline") {
        let slot = args.get(2).map(String::as_str).unwrap_or("");
        std::process::exit(claude_usage_tray_lib::statusline(slot));
    }
    claude_usage_tray_lib::run();
}
