mod koconf;
mod rsync_pattern;
mod embedded_code;

use rsync_pattern::RsyncPattern;

use std::process::exit;
use std::fmt::{Debug,Display};
use std::fs;
use std::io;
use std::io::{BufWriter,Write};
use std::borrow::{Borrow,BorrowMut,Cow};
use std::ffi::{OsStr,OsString};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;

fn non_panicky_unwrap<T, E: Display + Debug>(x: Result<T,E>) -> T {
    if cfg!(debug_assertions) {
        x.unwrap()
    }
    else {
        match x {
            Ok(x) => x,
            Err(e) => {
                eprintln!("{}", e);
                exit(1);
            }
        }
    }
}

#[derive(Debug)]
struct SeenRsyncPattern {
    seen: bool,
    problematic: bool,
    pattern: RsyncPattern,
}

impl SeenRsyncPattern {
    pub fn new(source: &[u8]) -> Result<SeenRsyncPattern, String> {
        Ok(SeenRsyncPattern {
            seen: false,
            problematic: false,
            pattern: RsyncPattern::new(source)?,
        })
    }
}

impl std::ops::Deref for SeenRsyncPattern {
    type Target = RsyncPattern;
    fn deref(&self) -> &RsyncPattern { &self.pattern }
}

#[derive(Debug,PartialEq,Eq)]
enum TestResult {
    /// A file or directory that was covered by `excludes`
    Excluded,
    /// A file or directory that was covered by `vetted`, and had no unvetted /
    /// unexcluded children.
    FullyVetted,
    /// A file that was not covered by `excludes` or `vetted`
    UnvettedFile(u64),
    /// A directory that was not covered by `excludes`, and either was not
    /// covered by `vetted` or had children that were not excluded or vetted.
    UnvettedDirectory(Vec<(Vec<u8>, TestResult)>),
    /// A directory that was not covered by `excludes`, and where an IO error
    /// occurred
    ErrorDirectory,
    /// A file or directory that is acting as a mount point. (The Knockout
    /// client will not traverse these by default.)
    Mount,
}

impl TestResult {
    pub fn output<W: io::Write>(&self, out: &mut W, name: &[u8])
                                -> io::Result<()> {
        match self {
            &TestResult::Excluded => {
                out.write_all(b"\"x")?;
                out_escaped_string(out.borrow_mut(), name.to_vec())?;
                out.write_all(b"\",")?;
            },
            &TestResult::FullyVetted => {
                out.write_all(b"\"v")?;
                out_escaped_string(out.borrow_mut(), name.to_vec())?;
                out.write_all(b"\",")?;
            },
            &TestResult::UnvettedFile(size) => {
                out.write_all(b"\"f")?;
                out_escaped_string(out.borrow_mut(), name.to_vec())?;
                out.write_all(b":")?;
                out.write_all(format!("{}", size).as_bytes())?;
                out.write_all(b"\",")?;
            },
            &TestResult::ErrorDirectory => {
                out.write_all(b"\"e")?;
                out_escaped_string(out.borrow_mut(), name.to_vec())?;
                out.write_all(b"\",")?;
            },
            &TestResult::Mount => {
                out.write_all(b"\"m")?;
                out_escaped_string(out.borrow_mut(), name.to_vec())?;
                out.write_all(b"\",")?;
            },
            &TestResult::UnvettedDirectory(ref children) => {
                out.write_all(b"[\"d")?;
                out_escaped_string(out.borrow_mut(), name.to_vec())?;
                out.write_all(b"\",")?;
                for (name, result) in children {
                    result.output(out.borrow_mut(), name)?;
                }
                out.write_all(b"],\n")?;
            },
        }
        Ok(())
    }
}

fn recursively_test(mut path: Cow<[u8]>, dev: u64,
                    excludes: &mut [SeenRsyncPattern],
                    vetted: &mut [SeenRsyncPattern],
                    errors: &mut Vec<u8>) -> TestResult {
    debug_assert!(!path.ends_with(b"/"));
    let metadata = fs::symlink_metadata(OsStr::from_bytes(path.borrow()));
    if let Ok(metadata) = metadata.as_ref() {
        // TODO: check for --no-one-file-system in `extras`, and disable this
        // check if it's found
        if metadata.dev() != dev { return TestResult::Mount }
    }
    let is_dir = metadata.as_ref().map(|x| x.is_dir()).unwrap_or(false);
    if is_dir { path.to_mut().push(b'/') }
    for exclude in excludes.iter_mut() {
        if exclude.matches(path.borrow()) {
            exclude.seen = true;
            return TestResult::Excluded
        }
    }
    // all the reasons a vet would previously be marked as problematic were
    // removed
    let vet_would_be_problematic = false;
    let mut dir_results = Vec::new();
    if is_dir {
        let buf = path.to_mut();
        let len_with_slash = buf.len();
        let iterator = match fs::read_dir(OsStr::from_bytes(buf)) {
            Ok(iterator) => iterator,
            Err(e) => {
                eprintln!("{}: {}", String::from_utf8_lossy(buf), e);
                return TestResult::ErrorDirectory
            }
        };
        for ent in iterator {
            match ent {
                Ok(ent) => {
                    buf.resize(len_with_slash, 0);
                    buf.extend_from_slice(ent.file_name().as_bytes());
                    if let None = ent.file_name().to_str() {
                        let warning =
                            format!("WARNING: filename of {:?} contains \
                                     invalid characters\n",
                                    OsStr::from_bytes(&buf));
                        errors.extend_from_slice(warning.as_bytes());
                    }
                    let result = recursively_test(Cow::Borrowed(&buf), dev, excludes, vetted, errors);
                    dir_results.push((ent.file_name().as_bytes().to_vec(),
                                      result));
                },
                Err(e) => {
                    eprintln!("{}", e);
                },
            }
        }
    }
    let mut is_vetted = false;
    for vet in vetted.iter_mut() {
        if vet.matches(path.borrow()) {
            vet.seen = true;
            if vet_would_be_problematic && !vet.problematic {
                vet.problematic = true;
                let warning =
                    format!("WARNING: `vetted` pattern {:?} is problematic\n",
                            OsStr::from_bytes(vet.get_original_form()));
                errors.extend_from_slice(warning.as_bytes());
            }
            is_vetted = true;
            break;
        }
    }
    if is_dir {
        if is_vetted && !vet_would_be_problematic {
            return TestResult::FullyVetted
        }
        else {
            return TestResult::UnvettedDirectory(dir_results)
        }
    }
    else {
        if is_vetted { return TestResult::FullyVetted }
        else { return TestResult::UnvettedFile(metadata.map(|x| x.len())
                                               .unwrap_or(0)) }
    }
}

fn out_escaped_string<W: io::Write>(mut out: W, mut bytes: Vec<u8>)
                                    -> io::Result<()> {
    const HEX_DIGITS: [u8; 16] = *b"0123456789ABCDEF";
    let mut n = 0;
    while n < bytes.len() {
        if bytes[n] == b'"' || bytes[n] == b'\\' {
            bytes.insert(n, b'\\');
            n += 2;
        }
        else if bytes[n] == b'\n' {
            bytes[n] = b'\\';
            bytes.insert(n+1, b'n');
            n += 2;
        }
        else if bytes[n] < b' ' {
            let b = bytes[n];
            bytes[n] = b'\\';
            bytes.insert(n+1, b'x');
            bytes.insert(n+2, HEX_DIGITS[(b >> 4) as usize]);
            bytes.insert(n+3, HEX_DIGITS[(b & 15) as usize]);
            n += 4;
        }
        else {
            n += 1;
        }
    }
    let str = String::from_utf8_lossy(&bytes);
    out.write_all(str.as_bytes())
}

fn main() {
    let args: Vec<OsString> = std::env::args_os().collect();
    if args.len() != 2 {
        eprintln!("Usage: knockout-exclude-check output.html");
        exit(1);
    }
    let koconf = non_panicky_unwrap(koconf::init());
    let sources: Vec<Vec<u8>> = match koconf.get("sources") {
        Err(_) => {
            eprintln!("The Knockout 'sources' configuration file doesn't exist \
                       or is inaccessible.\nCreate it before continuing.");
            exit(1);
        },
        Ok(x) => x,
    }.split(|x| *x == b'\n')
        .filter(|x| x.len() > 0)
        .filter(|x| x[0] != b'#')
        .map(|mut x| { while x.len() > 0 && x[0] == b'/' { x = &x[1..] } x })
        .map(|mut x| { while x.len() > 0 && x[x.len()-1] == b'/' { x = &x[..x.len()-1] } x })
        .map(|x| x.to_owned())
        .collect();
    if sources.is_empty() {
        eprintln!("`sources` is empty. If you really want to back up \
                   everything, you must list each filesystem you want to back \
                   up in `sources`.");
        exit(1);
    }
    let mut excludes: Vec<SeenRsyncPattern>
        = non_panicky_unwrap(koconf.get("excludes"))
        .split(|x| *x == b'\n')
        .filter(|x| x.len() > 0)
        .filter(|x| x[0] != b'#')
        .map(|x| non_panicky_unwrap(SeenRsyncPattern::new(x)))
        .collect();
    let mut vetted: Vec<SeenRsyncPattern>
        = koconf.get("vetted").unwrap_or(Vec::new())
        .split(|x| *x == b'\n')
        .filter(|x| x.len() > 0)
        .filter(|x| x[0] != b'#')
        .map(|x| non_panicky_unwrap(SeenRsyncPattern::new(x)))
        .collect();
    // we have to open the file now because we're about to chdir
    let mut output_file = BufWriter::new(non_panicky_unwrap(fs::File::create(&args[1])));
    embedded_code::write_header(&mut output_file).unwrap();
    std::env::set_current_dir("/").unwrap();
    let mut errors: Vec<u8> = Vec::new();
    output_file.write_all(b"\"use strict\";\nlet raw_tree = [").unwrap();
    for source in sources {
        let dev = fs::metadata(OsStr::from_bytes(&source))
            .map(|x| x.dev()).unwrap_or(0);
        let result = recursively_test(Cow::Borrowed(&source), dev,
                                      &mut excludes, &mut vetted, &mut errors);
        result.output(&mut output_file, &source).unwrap();
    }
    for exclude in &excludes {
        if !exclude.seen {
            errors.extend_from_slice(b"WARNING: unused `excludes` pattern:");
            errors.extend_from_slice(exclude.get_original_form());
            errors.push(b'\n');
        }
    }
    for vet in &vetted {
        if !vet.seen {
            errors.extend_from_slice(b"WARNING: unused `vetted` pattern:");
            errors.extend_from_slice(vet.get_original_form());
            errors.push(b'\n');
        }
    }
    {
        let stderr = std::io::stderr();
        stderr.lock().write_all(&errors).unwrap();
    }
    output_file.write_all(b"];\nlet errors = \"").unwrap();
    out_escaped_string(&mut output_file, errors).unwrap();
    output_file.write_all(b"\";\n").unwrap();
    embedded_code::write_footer(&mut output_file).unwrap();
}
