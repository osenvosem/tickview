//! Icon rendering: `crates/app/pack/tickview-mark.svg` -> raster PNGs.
//!
//! The SVG is the single source of truth for the app icon. `xtask icon --write`
//! regenerates the 1024 px master PNG that Linux packaging installs into
//! `hicolor/1024x1024/apps/` and that the macOS job feeds into `iconutil`.
//! Keeping the raster in the repo means CI never needs a vector renderer.

use std::path::Path;

/// Master raster edge, in pixels. 1024 covers the largest macOS iconset slot
/// (`icon_512x512@2x`) without upscaling.
const MASTER_PX: u32 = 1024;

/// Paths relative to the repository root.
const MARK_SVG: &str = "crates/app/pack/tickview-mark.svg";
const MARK_PNG: &str = "crates/app/pack/tickview-mark.png";

/// Renders `svg` to a PNG of `px` square pixels, in memory.
fn render(svg: &Path, px: u32) -> Result<Vec<u8>, String> {
    let data = std::fs::read(svg).map_err(|e| format!("read {}: {e}", svg.display()))?;
    let tree = resvg::usvg::Tree::from_data(&data, &resvg::usvg::Options::default())
        .map_err(|e| format!("parse {}: {e}", svg.display()))?;
    let size = tree.size();
    if size.width() <= 0.0 || size.height() <= 0.0 {
        return Err(format!("{} has no usable viewBox", svg.display()));
    }
    let scale = px as f32 / size.width().min(size.height());
    let mut pixmap = resvg::tiny_skia::Pixmap::new(px, px).ok_or("pixmap alloc")?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    if pixmap.pixels().iter().all(|p| p.alpha() == 0) {
        return Err(format!("{} rendered empty", svg.display()));
    }
    pixmap.encode_png().map_err(|e| format!("encode: {e}"))
}

fn repo_root() -> std::path::PathBuf {
    match std::env::var("CARGO_MANIFEST_DIR") {
        Ok(dir) => Path::new(&dir).join(".."),
        Err(_) => std::path::PathBuf::from("."),
    }
}

/// `xtask icon [--write]`
///
/// Without `--write` the master PNG is re-rendered and compared with the
/// committed one, so CI catches a stale icon instead of shipping it.
pub fn run(args: &[String]) -> i32 {
    let root = repo_root();
    let svg = root.join(MARK_SVG);
    let bytes = match render(&svg, MASTER_PX) {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("icon: {e}");
            return 1;
        }
    };
    let dest = root.join(MARK_PNG);
    if args.iter().any(|a| a == "--write") {
        match std::fs::write(&dest, &bytes) {
            Ok(()) => {
                println!(
                    "wrote {MARK_PNG} ({MASTER_PX}px, {} KiB)",
                    bytes.len() / 1024
                );
                return 0;
            }
            Err(e) => {
                eprintln!("icon: write {}: {e}", dest.display());
                return 1;
            }
        }
    }
    match std::fs::read(&dest) {
        Ok(have) if have == bytes => {
            println!(
                "{MARK_PNG} is current ({MASTER_PX}px, {} KiB)",
                bytes.len() / 1024
            );
            0
        }
        Ok(have) => {
            eprintln!(
                "{MARK_PNG} is stale ({} bytes on disk, {} rendered); run `cargo run -p xtask -- icon --write`",
                have.len(),
                bytes.len()
            );
            1
        }
        Err(_) => {
            eprintln!("{MARK_PNG} is missing; run `cargo run -p xtask -- icon --write`");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mark_renders_opaque_centre_and_clear_corner() {
        let bytes = render(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join(MARK_SVG),
            64,
        )
        .expect("render mark");
        let img = image::load_from_memory(&bytes)
            .expect("decode")
            .into_rgba8();
        assert_eq!(img.dimensions(), (64, 64));
        assert!(
            img.get_pixel(32, 32).0[3] > 200,
            "the tile centre must be opaque"
        );
        assert_eq!(
            img.get_pixel(0, 0).0[3],
            0,
            "the rounded corner must stay transparent"
        );
    }

    #[test]
    fn committed_master_png_matches_the_svg() {
        // Packaging installs this file; a stale raster ships a wrong icon.
        assert_eq!(run(&[]), 0);
    }
}
