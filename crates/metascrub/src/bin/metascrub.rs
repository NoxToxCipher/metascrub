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
        --in-place       Overwrite each input file. Note: this replaces the file
                         via a rename; it does NOT shred the previous contents,
                         whose old disk blocks may still be recoverable.
        --suffix <S>     Suffix for the output name (default: clean)
        --random-name    Name each cleaned copy with 24 random characters,
                         keeping the extension. The file name is metadata too:
                         this drops the date, place or camera prefix it carried.
        --keep-icc       Keep embedded colour profiles
        --keep-rotation  Keep the EXIF orientation tag, rebuilt from scratch,
                         so photos do not display sideways
        --no-recurse     Do not sanitize images embedded in documents
        --verify         Re-scan the cleaned output to confirm nothing removable
                         survived, and confirm the clean is reproducible
        --no-sidecars    Do not look for metadata sidecar files (.xmp, .thm, .aae)
                         next to each input
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
        // Computed once, here, and reused for both the write and the printed
        // "-> destination" line. A random name generated twice would not match.
        let dst = opts.destination(path);
        match process(path, &dst, &opts) {
            Ok(report) => {
                if report.assurance == Assurance::None {
                    worst = worst.max(2);
                }
                reports.push((path.clone(), report, dst));
            }
            Err(e) => {
                eprintln!("metascrub: {}: {e}", path.display());
                worst = worst.max(1);
            }
        }

        // A photo often travels with a metadata sidecar (.xmp, .thm, .aae) that
        // carries GPS, dates and author on its own and is not cleaned by cleaning
        // the photo. Find and handle those too, unless told not to.
        if opts.sidecars {
            for sc in sidecar_paths(path) {
                if opts.inputs.iter().any(|p| p == &sc) {
                    continue; // the user listed it explicitly; don't do it twice
                }
                let dst = opts.destination(&sc);
                match process(&sc, &dst, &opts) {
                    Ok(report) => {
                        if report.assurance == Assurance::None {
                            worst = worst.max(2);
                        }
                        reports.push((sc, report, dst));
                    }
                    Err(e) => {
                        eprintln!("metascrub: {}: {e}", sc.display());
                        worst = worst.max(1);
                    }
                }
            }
        }
    }

    if opts.json {
        print_json(&reports);
    } else {
        for (path, report, dst) in &reports {
            print_human(path, report, dst, &opts);
        }
    }
    ExitCode::from(worst)
}

/// Metadata sidecar extensions written beside photos by cameras and editors.
/// `.xmp` and `.thm` this tool can clean; `.aae` and the editor formats it will
/// flag as unsupported, which is the honest outcome and still tells the user
/// the file is there.
const SIDECAR_EXTS: &[&str] = &["xmp", "thm", "aae", "pp3", "dop", "on1", "repair"];

/// Existing sidecar files that sit beside `path`, matched two ways: replacing
/// the extension (`photo.xmp`) and appending it (`photo.jpg.xmp`), since both
/// conventions are in use. Case-insensitive on the extension.
fn sidecar_paths(path: &Path) -> Vec<PathBuf> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path.file_stem().and_then(|s| s.to_str());
    let full = path.file_name().and_then(|s| s.to_str());
    let mut out: Vec<PathBuf> = Vec::new();
    // Deduplicate on the canonical path, so a case-insensitive filesystem does
    // not hand back `photo.xmp` and `photo.XMP` as two different files.
    let mut seen = std::collections::BTreeSet::new();
    let canon = |p: &Path| std::fs::canonicalize(p).ok();
    let self_canon = canon(path);
    for ext in SIDECAR_EXTS {
        for cand in [
            stem.map(|s| dir.join(format!("{s}.{ext}"))),
            stem.map(|s| dir.join(format!("{s}.{}", ext.to_uppercase()))),
            full.map(|s| dir.join(format!("{s}.{ext}"))),
            full.map(|s| dir.join(format!("{s}.{}", ext.to_uppercase()))),
        ]
        .into_iter()
        .flatten()
        {
            if !cand.is_file() {
                continue;
            }
            let key = canon(&cand);
            if key.is_some() && key == self_canon {
                continue; // the input itself
            }
            let dedup = key.clone().unwrap_or_else(|| cand.clone());
            if seen.insert(dedup) {
                out.push(cand);
            }
        }
    }
    out
}

/// Largest file the tool will read into memory, checked before reading so an
/// enormous file is refused rather than loaded. No real photograph or document
/// approaches 2 GB.
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024 * 1024;

fn process(path: &Path, dst: &Path, opts: &Options) -> metascrub::Result<Report> {
    if let Ok(meta) = std::fs::metadata(path) {
        if meta.len() > MAX_FILE_BYTES {
            return Err(metascrub::Error::TooLarge { len: meta.len(), limit: MAX_FILE_BYTES });
        }
    }
    let input = std::fs::read(path)?;
    let result = if opts.verify {
        metascrub::sanitize_verified(&input, &opts.policy)?
    } else {
        metascrub::sanitize(&input, &opts.policy)?
    };

    if !opts.dry_run && result.report.assurance != Assurance::None {
        write_atomic(dst, &result.data)?;
    }
    Ok(result.report)
}

/// Write cleaned bytes through the library's single hardened writer, so the
/// atomic-rename, unpredictable-name, `create_new` behaviour is not
/// reimplemented here where it could drift weaker.
fn write_atomic(dst: &Path, data: &[u8]) -> metascrub::Result<()> {
    metascrub::write_atomic(dst, data)
}

fn print_human(path: &Path, report: &Report, dst: &Path, opts: &Options) {
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
    if !report.retained.is_empty() {
        println!("  STILL IN THE FILE (could not be removed without corrupting it):");
        for r in &report.retained {
            println!("    · {}", r.what);
            println!("        reveals: {}", r.reveals);
        }
    }
    for warning in &report.warnings {
        println!("  ! {warning}");
    }
    if let Some(v) = report.verification {
        if v.passed() {
            println!("  \u{2713} verified: re-scan of the output found nothing removable; clean is reproducible");
        } else if !v.output_reinspected_clean {
            println!("  \u{2717} VERIFICATION FAILED: the output still contains removable metadata \u{2014} do not trust it");
        } else {
            println!("  \u{2717} VERIFICATION FAILED: cleaning the same file twice gave different output (non-deterministic)");
        }
    }
    if !opts.dry_run {
        println!("  -> {}", dst.display());
    }
}

/// Hand-rolled JSON, so the tool carries no serialization dependency.
fn print_json(reports: &[(PathBuf, Report, PathBuf)]) {
    println!("[");
    for (i, (path, report, _dst)) in reports.iter().enumerate() {
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
        println!(r#"    "retained": ["#);
        for (j, r) in report.retained.iter().enumerate() {
            let comma = if j + 1 < report.retained.len() { "," } else { "" };
            println!(
                r#"      {{"what": "{}", "reveals": "{}"}}{comma}"#,
                escape(&r.what),
                escape(&r.reveals),
            );
        }
        println!("    ],");
        if let Some(v) = report.verification {
            println!(
                r#"    "verification": {{"passed": {}, "output_reinspected_clean": {}, "deterministic": {}}},"#,
                v.passed(),
                v.output_reinspected_clean,
                v.deterministic,
            );
        }
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
    verify: bool,
    sidecars: bool,
    in_place: bool,
    suffix: String,
    out: Option<PathBuf>,
    random_name: bool,
}

impl Options {
    /// Returns `Ok(None)` when the run is over (help or version was printed).
    fn parse(args: &[String]) -> Result<Option<Self>, String> {
        let mut opts = Options {
            inputs: Vec::new(),
            policy: Policy { max_input_bytes: Some(MAX_FILE_BYTES), ..Policy::default() },
            dry_run: false,
            quiet: false,
            json: false,
            verify: false,
            sidecars: true,
            in_place: false,
            suffix: "clean".to_string(),
            out: None,
            random_name: false,
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
                "--verify" => opts.verify = true,
                "--no-sidecars" => opts.sidecars = false,
                "--in-place" => opts.in_place = true,
                "--keep-icc" => opts.policy.color_profile = ColorProfile::Keep,
                "--keep-rotation" => opts.policy.orientation = Orientation::PreserveMinimal,
                "--no-recurse" => opts.policy.recurse_embedded = false,
                "--random-name" => opts.random_name = true,
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
        // A random name only means anything for the automatic destination. With
        // --out you already named the file; with --in-place there is no new name.
        if opts.random_name && opts.out.is_some() {
            return Err("--random-name and --out are mutually exclusive".to_string());
        }
        if opts.random_name && opts.in_place {
            return Err("--random-name and --in-place are mutually exclusive".to_string());
        }
        Ok(Some(opts))
    }

    /// Where the cleaned copy goes.
    ///
    /// For `--random-name` this rolls a fresh name, so it must be called exactly
    /// once per file and the result reused; the caller in `main` does that.
    fn destination(&self, src: &Path) -> PathBuf {
        if let Some(out) = &self.out {
            return out.clone();
        }
        if self.in_place {
            return src.to_path_buf();
        }
        if self.random_name {
            return random_destination(src);
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

/// A fresh 24-character random name beside `src`, keeping the extension (lower-
/// cased) so the file still opens. The name is regenerated if it already exists,
/// because the write is a plain rename that would otherwise replace a file; a
/// collision on 24 base32 characters is only a theoretical worry, but a cleaned
/// file is not worth losing to one.
fn random_destination(src: &Path) -> PathBuf {
    let dir = src.parent().unwrap_or_else(|| Path::new("."));
    let ext = src.extension().map(|e| e.to_string_lossy().to_lowercase());
    for _ in 0..16 {
        let stem = metascrub::random_stem(24);
        let name = match &ext {
            Some(e) if !e.is_empty() => format!("{stem}.{e}"),
            _ => stem,
        };
        let cand = dir.join(name);
        if !cand.exists() {
            return cand;
        }
    }
    dir.join(metascrub::random_stem(24))
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
        assert!(
            parse(&["--random-name", "-o", "x.jpg", "a.jpg"]).is_err(),
            "--random-name with --out"
        );
        assert!(
            parse(&["--random-name", "--in-place", "a.jpg"]).is_err(),
            "--random-name with --in-place"
        );
    }

    #[test]
    fn a_random_name_is_distinct_keeps_the_extension_and_varies() {
        let opts = parse(&["--random-name", "IMG_20230715_Berlin.JPG"]).unwrap().unwrap();
        assert!(opts.random_name);
        let dst = opts.destination(Path::new("IMG_20230715_Berlin.JPG"));
        // Nothing of the original name survives, and the extension is lower-cased.
        assert_eq!(dst.extension().and_then(|e| e.to_str()), Some("jpg"));
        let stem = dst.file_stem().and_then(|s| s.to_str()).unwrap();
        assert_eq!(stem.len(), 24);
        assert!(!stem.contains("Berlin") && !stem.contains("2023"));
        assert!(stem.chars().all(|c| matches!(c, 'a'..='z' | '2'..='7')));
        // A second roll is a different name, so it is genuinely random.
        let dst2 = opts.destination(Path::new("IMG_20230715_Berlin.JPG"));
        assert_ne!(dst, dst2);
    }

    #[test]
    fn a_random_name_without_an_extension_is_just_the_token() {
        let opts = parse(&["--random-name", "scan"]).unwrap().unwrap();
        let dst = opts.destination(Path::new("scan"));
        assert_eq!(dst.extension(), None);
        assert_eq!(dst.file_name().and_then(|s| s.to_str()).unwrap().len(), 24);
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
