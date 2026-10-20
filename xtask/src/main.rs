//! `xtask` — dev tooling. Subcommands: `parity` (manifest validation and
//! status tables), `rescrape` (re-fetch pine-facade study metadata).

mod parity;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("parity") => std::process::exit(parity::run(&args[2..])),
        Some("rescrape") => {
            println!("{}: not wired until Task 11", args[1]);
            std::process::exit(0);
        }
        _ => {
            eprintln!("usage: xtask <parity|rescrape> [--write]");
            std::process::exit(2);
        }
    }
}
