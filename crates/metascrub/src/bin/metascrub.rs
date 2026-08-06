//! `metascrub` — strip metadata from files, from the command line.
//!
//! Primarily a way to exercise the library against real files from real
//! cameras and real word processors, which is the only way to find out what a
//! format actually contains as opposed to what its specification says.
//!
//! Arguments are parsed by hand rather than with a crate. The surface is a
//! dozen flags, and this keeps the dependency list at exactly the set the
//! library itself needs, which matters for a tool people may want to audit
//! before trusting it with their photographs.

use metascrub::{Assurance, ColorProfile, Orientation, Policy, Report};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "\
metascrub — remove metadata from images and documents

USAGE:
    metascrub [OPTIONS] <FILE>...

By default each file is written alongside the original with a .clean suffix,
so nothing is overwritten until you ask for it.

OPTIONS:
    -n, --dry-run        Report what would be removed, write nothing
    -o, --out <PATH>     Write to PATH (one input file only)
        --in-place       Overwrite each input file
        --suffix <S>     Suffix for the output name (default: clean)
        --keep-icc       Keep embedded colour profiles
        --keep-rotation  Keep the EXIF orientation tag, rebuilt from scratch,
                         so photos do not display sideways
        --no-recurse     Do not sanitize images embedded in documents
        --json           Machine-readable output
    -q, --quiet          Only report problems
    -h, --help           Show this message
    -V, --version        Show the version

EXIT STATUS:
    0  every file was handled
    1  a file failed to parse, or could not be read or written
    2  a file was left unsanitized because its format is not supported
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let opts = match Options::parse(&args) {
        Ok(Some(opts)) => opts,
        Ok(None) => return ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("metascrub: {msg}");
            eprintln!("try 'metascrub --help'");
            return ExitCode::from(1);
        }
    };

    let mut worst = 0u8;
    let mut reports = Vec::new();

    for path in &opts.inputs {
        match process(path, &opts) {
            Ok(report) => {
                if report.assurance == Assurance::None {
                    worst = worst.max(2);
                }
                reports.push((path.clone(), report));
            }
            Err(e) => {
                eprintln!("metascrub: {}: {e}", path.display());
                worst = 1;
            }
        }
    }

    if opts.json {
        print_json(&reports);
    } else {
        for (path, report) in &reports {
            print_human(path, report, &opts);
        }
    }
    ExitCode::from(worst)
}

fn process(path: &Path, opts: &Options) -> metascrub::Result<Report> {
    let input = std::fs::read(path)?;
    let result = metascrub::sanitize(&input, &opts.policy)?;

    if !opts.dry_run && result.report.assurance != Assurance::None {
        write_atomic(&opts.destination(path), &result.data)?;
    }
    Ok(result.report)
}

/// Write through a temporary file in the same directory, then rename.
///
/// A direct write is not atomic: interrupt it and the leftover is a truncated
/// file carrying a name that says it was cleaned, which the user has no reason
/// to distrust. With `--in-place` a direct write also destroys the original
/// before the replacement is complete. Renaming within a directory is atomic on
/// both Unix and Windows, so the destination is either the old file or the
/// whole new one.
fn write_atomic(dst: &Path, data: &[u8]) -> std::io::Result<()> {
    use std::io::Write;

    let dir = dst.parent().unwrap_or_else(|| Path::new("."));
    let name = dst.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = dir.join(format!(".{name}.{}.metascrub", std::process::id()));

    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        // The cleaned file is the user's photograph; it should not be
        // world-readable even for the moment before the rename.
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }

    let mut file = opts.open(&tmp)?;
    let written = file.write_all(data).and_then(|()| file.sync_all());
    drop(file);
    if let Err(e) = written {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    if let Err(e) = std::fs::rename(&tmp, dst) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(())
}

fn print_human(path: &Path, report: &Report, opts: &Options) {
    let name = path.display();

    if report.assurance == Assurance::None {
        eprintln!("{name}: NOT SANITIZED, {}", report.format);
        for warning in &report.warnings {
            eprintln!("  ! {warning}");
        }
        return;
    }
    if opts.quiet && report.is_clean() && report.warnings.is_empty() {
        return;
    }

    println!("{name}: {}", report.summary());
    if report.found_location {
        // The one finding worth breaking the list format for.
        println!("  ** this file recorded where it was taken **");
    }
    for item in &report.removed {
        let size = if item.bytes > 0 { format!(", {} bytes", item.bytes) } else { String::new() };
        println!("  - {} at {}{size}", item.kind, item.location);
    }
    for warning in &report.warnings {
        println!("  ! {warning}");
    }
    if !opts.dry_run {
        println!("  -> {}", opts.destination(path).display());
    }
}

/// Hand-rolled JSON, so the tool carries no serialization dependency.
fn print_json(reports: &[(PathBuf, Report)]) {
    println!("[");
    for (i, (path, report)) in reports.iter().enumerate() {
        let comma = if i + 1 < reports.len() { "," } else { "" };
        println!("  {{");
        println!(r#"    "file": "{}","#, escape(&path.display().to_string()));
        println!(r#"    "format": "{}","#, escape(&report.format.to_string()));
        println!(r#"    "assurance": "{}","#, escape(&report.assurance.to_string()));
        println!(r#"    "found_location": {},"#, report.found_location);
        println!(r#"    "input_bytes": {},"#, report.input_len);
        println!(r#"    "output_bytes": {},"#, report.output_len);
        println!(r#"    "removed": ["#);
        for (j, item) in report.removed.iter().enumerate() {
            let comma = if j + 1 < report.removed.len() { "," } else { "" };
            println!(
                r#"      {{"kind": "{}", "location": "{}", "bytes": {}}}{comma}"#,
                escape(&item.kind.to_string()),
                escape(&item.location),
                item.bytes,
            );
        }
        println!("    ],");
        println!(r#"    "warnings": ["#);
        for (j, warning) in report.warnings.iter().enumerate() {
            let comma = if j + 1 < report.warnings.len() { "," } else { "" };
            println!(r#"      "{}"{comma}"#, escape(warning));
        }
        println!("    ]");
        println!("  }}{comma}");
    }
    println!("]");
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

struct Options {
    inputs: Vec<PathBuf>,
    policy: Policy,
    dry_run: bool,
    quiet: bool,
    json: bool,
    in_place: bool,
    suffix: String,
    out: Option<PathBuf>,
}

impl Options {
    /// Returns `Ok(None)` when the run is over (help or version was printed).
    fn parse(args: &[String]) -> Result<Option<Self>, String> {
        let mut opts = Options {
            inputs: Vec::new(),
            policy: Policy::default(),
            dry_run: false,
            quiet: false,
            json: false,
            in_place: false,
            suffix: "clean".to_string(),
            out: None,
        };

        let mut i = 0;
        let mut only_files = false;
        while i < args.len() {
            let arg = args[i].as_str();
            i += 1;

            if only_files || !arg.starts_with('-') || arg == "-" {
                opts.inputs.push(PathBuf::from(arg));
                continue;
            }
            let mut value = |name: &str| -> Result<String, String> {
                let v = args.get(i).ok_or_else(|| format!("{name} needs a value"))?;
                i += 1;
                Ok(v.clone())
            };

            match arg {
                "--" => only_files = true,
                "-h" | "--help" => {
                    print!("{USAGE}");
                    return Ok(None);
                }
                "-V" | "--version" => {
                    println!("metascrub {}", env!("CARGO_PKG_VERSION"));
                    return Ok(None);
                }
                "-n" | "--dry-run" => opts.dry_run = true,
                "-q" | "--quiet" => opts.quiet = true,
                "--json" => opts.json = true,
                "--in-place" => opts.in_place = true,
                "--keep-icc" => opts.policy.color_profile = ColorProfile::Keep,
                "--keep-rotation" => opts.policy.orientation = Orientation::PreserveMinimal,
                "--no-recurse" => opts.policy.recurse_embedded = false,
                "--suffix" => opts.suffix = value("--suffix")?,
                "-o" | "--out" => opts.out = Some(PathBuf::from(value("--out")?)),
                other => return Err(format!("unknown option '{other}'")),
            }
        }

        if opts.inputs.is_empty() {
            return Err("no input files".to_string());
        }
        if opts.out.is_some() && opts.inputs.len() > 1 {
            return Err("--out takes a single input file".to_string());
        }
        if opts.out.is_some() && opts.in_place {
            return Err("--out and --in-place are mutually exclusive".to_string());
        }
        Ok(Some(opts))
    }

    /// Where the cleaned copy goes.
    fn destination(&self, src: &Path) -> PathBuf {
        if let Some(out) = &self.out {
            return out.clone();
        }
        if self.in_place {
            return src.to_path_buf();
        }
        // photo.jpg becomes photo.clean.jpg, keeping the extension so the file
        // still opens by double-click.
        match src.extension().and_then(|e| e.to_str()) {
            Some(ext) => src.with_extension(format!("{}.{ext}", self.suffix)),
            None => {
                let mut name = src.as_os_str().to_os_string();
                name.push(format!(".{}", self.suffix));
                PathBuf::from(name)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Option<Options>, String> {
        Options::parse(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn the_default_policy_is_the_strict_one() {
        let opts = parse(&["a.jpg"]).unwrap().unwrap();
        assert_eq!(opts.policy.orientation, Orientation::Drop);
        assert_eq!(opts.policy.color_profile, ColorProfile::Drop);
        assert!(opts.policy.recurse_embedded);
        assert!(!opts.dry_run);
    }

    #[test]
    fn each_keep_flag_relaxes_exactly_one_thing() {
        let opts =
            parse(&["--keep-icc", "--keep-rotation", "--no-recurse", "a.jpg"]).unwrap().unwrap();
        assert_eq!(opts.policy.orientation, Orientation::PreserveMinimal);
        assert_eq!(opts.policy.color_profile, ColorProfile::Keep);
        assert!(!opts.policy.recurse_embedded);
    }

    #[test]
    fn the_default_destination_never_overwrites_the_input() {
        let opts = parse(&["holiday.jpg"]).unwrap().unwrap();
        assert_eq!(opts.destination(Path::new("holiday.jpg")), Path::new("holiday.clean.jpg"));
        // An extensionless file still gets a distinct name.
        assert_eq!(opts.destination(Path::new("scan")), Path::new("scan.clean"));
    }

    #[test]
    fn in_place_and_out_choose_the_destination() {
        let opts = parse(&["--in-place", "a.jpg"]).unwrap().unwrap();
        assert_eq!(opts.destination(Path::new("a.jpg")), Path::new("a.jpg"));

        let opts = parse(&["-o", "out.png", "a.png"]).unwrap().unwrap();
        assert_eq!(opts.destination(Path::new("a.png")), Path::new("out.png"));
    }

    #[test]
    fn a_custom_suffix_is_honoured() {
        let opts = parse(&["--suffix", "scrubbed", "a.jpg"]).unwrap().unwrap();
        assert_eq!(opts.destination(Path::new("a.jpg")), Path::new("a.scrubbed.jpg"));
    }

    #[test]
    fn contradictory_and_incomplete_invocations_are_rejected() {
        assert!(parse(&[]).is_err(), "no inputs");
        assert!(parse(&["-o", "x.jpg", "a.jpg", "b.jpg"]).is_err(), "--out with two inputs");
        assert!(parse(&["-o", "x.jpg", "--in-place", "a.jpg"]).is_err(), "--out with --in-place");
        assert!(parse(&["--suffix"]).is_err(), "--suffix with no value");
        assert!(parse(&["--nonsense", "a.jpg"]).is_err(), "unknown option");
    }

    #[test]
    fn a_double_dash_lets_a_filename_start_with_a_dash() {
        let opts = parse(&["--", "--weird-name.jpg"]).unwrap().unwrap();
        assert_eq!(opts.inputs, vec![PathBuf::from("--weird-name.jpg")]);
    }

    #[test]
    fn help_and_version_end_the_run_without_needing_a_file() {
        assert!(parse(&["--help"]).unwrap().is_none());
        assert!(parse(&["-V"]).unwrap().is_none());
    }

    #[test]
    fn json_strings_are_escaped() {
        assert_eq!(escape(r#"a "quoted" \path"#), r#"a \"quoted\" \\path"#);
        assert_eq!(escape("line\nbreak\ttab"), "line\\nbreak\\ttab");
        assert_eq!(escape("bell\u{7}"), "bell\\u0007");
    }
}
