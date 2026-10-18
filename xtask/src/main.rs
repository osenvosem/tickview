//! `xtask` — dev tooling. Subcommands: `parity` (registry vs scraped meta),
//! `rescrape` (re-fetch pine-facade study metadata). Wired in Task 11.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("parity") | Some("rescrape") => {
            println!("{}: not wired until Task 11", args[1]);
            std::process::exit(0);
        }
        _ => {
            eprintln!("usage: xtask <parity|rescrape>");
            std::process::exit(2);
        }
    }
}
