//! `xtask` — dev tooling. Subcommands: `parity` (manifest validation and
//! status tables), `icon` (app-icon raster from the committed SVG), `rescrape`
//! (placeholder for re-fetching pine-facade study metadata).

mod icon;
mod parity;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("parity") => std::process::exit(parity::run(&args[2..])),
        Some("icon") => std::process::exit(icon::run(&args[2..])),
        Some("rescrape") => {
            eprintln!(
                "rescrape is not implemented; captured study metadata lives in \
                 docs/evidence/tv/studies/"
            );
            std::process::exit(2);
        }
        _ => {
            eprintln!("usage: xtask <parity|icon> [--write]");
            std::process::exit(2);
        }
    }
}
