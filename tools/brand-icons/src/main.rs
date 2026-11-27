//! Draws the brand icons listed in `assets/icons/simple-icons.txt` from a simple-icons package, in the style of the
//! Amazon Q icons next to them. Run it through `scripts/brand-icons.sh` (`just brand-icons`), which pins the version.
//!
//! Usage: brand-icons <simple-icons package dir> <manifest> <template.rgba> <output dir>

mod catalogue;
mod colour;
mod icon;
mod manifest;

use catalogue::Catalogue;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [package, manifest, template, out] = &args[..] else {
        eprintln!("usage: brand-icons <simple-icons package dir> <manifest> <template.rgba> <output dir>");
        return ExitCode::from(2);
    };
    match run(Path::new(package), Path::new(manifest), Path::new(template), Path::new(out)) {
        Ok(count) => {
            println!("drew {count} brand icons into {out}");
            ExitCode::SUCCESS
        }
        Err(problem) => {
            eprintln!("brand-icons: {problem}");
            ExitCode::FAILURE
        }
    }
}

/// Checks every manifest entry before drawing any, so a failed run writes nothing. Writes only `<slug>.png` files.
fn run(package: &Path, manifest: &Path, template: &Path, out: &Path) -> Result<usize, String> {
    let entries = manifest::parse(&read(manifest)?).map_err(|e| format!("{}: {e}", manifest.display()))?;
    let catalogue = Catalogue::from_json(&read(&package.join("data/simple-icons.json"))?)?;
    let problems: Vec<String> = entries.iter().filter_map(|entry| catalogue.check(entry).err()).collect();
    if !problems.is_empty() {
        return Err(problems.join("\n"));
    }
    let template = fs::read(template).map_err(|e| format!("{}: {e}", template.display()))?;
    for entry in &entries {
        let svg = read(&package.join(format!("icons/{}.svg", entry.slug)))?;
        let png = icon::drawn(&svg, entry.colour, &template).map_err(|e| format!("{}: {e}", entry.slug))?;
        let path = out.join(format!("{}.png", entry.slug));
        fs::write(&path, png).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(entries.len())
}

fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}
