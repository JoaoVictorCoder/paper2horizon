use paper2horizon::{analysis, generate};
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
paper2horizon: turns a Paper/Bukkit plugin .jar into a horizon-plugin-api crate to finish porting

Usage: paper2horizon <plugin.jar> [options]

Options:
  -o, --out <dir>      Where to write the crate (default: the plugin's name, lower case)
      --api-path <dir>  Use a local horizon-server-plugins api/ folder instead of the git dependency
      --report          Only print PORTING.md, write nothing
  -f, --force           Write into a folder that isn't empty
  -h, --help            This text";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("paper2horizon: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let mut jar = None;
    let mut out = None;
    let mut api = generate::ApiSource::Git;
    let mut report = false;
    let mut force = false;
    while let Some(a) = args.next() {
        match a.as_str() {
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            "-o" | "--out" => out = Some(PathBuf::from(args.next().ok_or("--out needs a folder")?)),
            "--api-path" => api = generate::ApiSource::Path(args.next().ok_or("--api-path needs a folder")?),
            "--report" => report = true,
            "-f" | "--force" => force = true,
            s if s.starts_with('-') => return Err(format!("unknown option {s}\n\n{USAGE}")),
            s => {
                if jar.replace(PathBuf::from(s)).is_some() {
                    return Err(format!("one jar at a time\n\n{USAGE}"));
                }
            }
        }
    }
    let jar = jar.ok_or(USAGE)?;
    let jar_name = jar.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let analysis = analysis::analyze(analysis::read_jar(&jar)?)?;
    if report {
        println!("{}", generate::porting_md(&analysis, &jar_name));
        return Ok(());
    }
    let out = out.unwrap_or_else(|| PathBuf::from(generate::crate_name(&analysis.description.name)));
    if out.exists() && !force && std::fs::read_dir(&out).map(|mut d| d.next().is_some()).unwrap_or(true) {
        return Err(format!("{} isn't empty (--force writes into it anyway)", out.display()));
    }
    // A relative --api-path is relative to here; the crate needs it relative to itself.
    if let generate::ApiSource::Path(p) = &api {
        let abs = std::fs::canonicalize(p).map_err(|e| format!("{p}: {e}"))?;
        api = generate::ApiSource::Path(abs.display().to_string());
    }
    let written = generate::write_crate(&analysis, &jar_name, &out, &api)?;
    println!(
        "{} {}: {} listeners, {} commands → {}/ ({} files). Next: {}/PORTING.md",
        analysis.description.name,
        analysis.description.version,
        analysis.listeners.len(),
        analysis.commands.len(),
        out.display(),
        written.len(),
        out.display()
    );
    Ok(())
}
