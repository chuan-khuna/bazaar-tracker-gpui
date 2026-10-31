//! `logs/tracker.log` in the data folder, written through `tracing`. The file rotates by size,
//! keeping the current file and a few backups (`tracker.log.1` is the newest).

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tracing_subscriber::fmt::MakeWriter;

pub const FILE_NAME: &str = "tracker.log";
/// The log rotates once it would pass this many bytes.
pub const MAX_BYTES: u64 = 1024 * 1024;
/// How many old logs are kept beside the current one.
pub const BACKUPS: usize = 3;

/// A log file that moves itself to `<name>.1` (and `.1` to `.2`, ...) once it would pass `max_bytes`,
/// dropping the oldest of `backups` old files.
pub struct RotatingFile {
    path: PathBuf,
    max_bytes: u64,
    backups: usize,
    file: Option<File>,
    size: u64,
}

impl RotatingFile {
    pub fn open(path: PathBuf, max_bytes: u64, backups: usize) -> io::Result<Self> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let size = file.metadata()?.len();
        Ok(Self { path, max_bytes, backups, file: Some(file), size })
    }

    fn backup_path(&self, number: usize) -> PathBuf {
        let mut name = self.path.clone().into_os_string();
        name.push(format!(".{number}"));
        PathBuf::from(name)
    }

    fn rotate(&mut self) -> io::Result<()> {
        // Close the file first: Windows will not rename an open one.
        self.file = None;
        if self.backups == 0 {
            fs::remove_file(&self.path)?;
        } else {
            let _ = fs::remove_file(self.backup_path(self.backups));
            for number in (1..self.backups).rev() {
                let from = self.backup_path(number);
                if from.exists() {
                    fs::rename(from, self.backup_path(number + 1))?;
                }
            }
            fs::rename(&self.path, self.backup_path(1))?;
        }
        self.file = Some(OpenOptions::new().create(true).append(true).open(&self.path)?);
        self.size = 0;
        Ok(())
    }
}

impl Write for RotatingFile {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.size > 0 && self.size + buffer.len() as u64 > self.max_bytes {
            // A failed rotation keeps appending to the same file rather than losing lines.
            let _ = self.rotate();
            if self.file.is_none() {
                self.file = Some(OpenOptions::new().create(true).append(true).open(&self.path)?);
            }
        }
        let file = self.file.as_mut().expect("the log file is open");
        let written = file.write(buffer)?;
        self.size += written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.file.as_mut() {
            Some(file) => file.flush(),
            None => Ok(()),
        }
    }
}

/// Hands `tracing` a shared handle to the rotating file.
#[derive(Clone)]
struct SharedWriter(Arc<Mutex<RotatingFile>>);

impl Write for SharedWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).write(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).flush()
    }
}

impl<'a> MakeWriter<'a> for SharedWriter {
    type Writer = SharedWriter;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// The first line of each launch's log.
pub fn banner() -> String {
    format!("Bazaar Tracker {} started (process {})", env!("CARGO_PKG_VERSION"), std::process::id())
}

/// Send `tracing` to `<data_dir>/logs/tracker.log`, start with the banner, and log panics.
pub fn init(data_dir: &Path) -> io::Result<()> {
    let file = RotatingFile::open(data_dir.join("logs").join(FILE_NAME), MAX_BYTES, BACKUPS)?;
    let subscriber = tracing_subscriber::fmt().with_writer(SharedWriter(Arc::new(Mutex::new(file)))).with_ansi(false).finish();
    // Only fails if a subscriber is already set, which leaves that one in charge.
    if tracing::subscriber::set_global_default(subscriber).is_ok() {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            tracing::error!("panic: {info}");
            previous(info);
        }));
    }
    tracing::info!("{}", banner());
    Ok(())
}

#[cfg(test)]
#[path = "logging_tests.rs"]
mod tests;
