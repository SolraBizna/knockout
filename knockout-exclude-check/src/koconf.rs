use std::env::var_os;
use std::{fs,io};
use std::io::Read;
use std::path::PathBuf;

pub fn get_knockout_dir() -> Result<PathBuf, String> {
    if let Some(str) = var_os("KNOCKOUT_DIR") {
        return Ok(PathBuf::from(str))
    }
    if let Some(home) = var_os("HOME") {
        // Rust provides `home_dir` mainly to support Windows's complicated
        // home directory finding logic. We, on the other hand, are trying to
        // mirror the exact logic in the `knockout-client.sh` script.
        let mut candidate_dir = home;
        candidate_dir.push("/.knockout");
        if let Ok(metadata) = fs::metadata(candidate_dir.as_os_str()) {
            if metadata.file_type().is_dir() {
                return Ok(PathBuf::from(candidate_dir))
            }
        }
    }
    if let Ok(metadata) = fs::metadata("/etc/knockout") {
        if metadata.file_type().is_dir() {
            return Ok(PathBuf::from("/etc/knockout"))
        }
    }
    Err("Could not find the Knockout configuration directory. Try setting \
         KNOCKOUT_DIR in the environment, exactly as you would for the \
         Knockout client.".to_owned())
}

#[derive(Debug,Clone)]
pub struct KoConf {
    knockout_dir: PathBuf
}

impl KoConf {
    pub fn open(&self, key: &str) -> io::Result<fs::File> {
        let path = self.knockout_dir.join(key);
        fs::File::open(path)
    }
    pub fn get(&self, key: &str) -> io::Result<Vec<u8>> {
        let mut file = self.open(key)?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;
        Ok(buf)
    }
}

pub fn init() -> Result<KoConf, String> {
    let knockout_dir = get_knockout_dir()?;
    Ok(KoConf {
        knockout_dir
    })
}
