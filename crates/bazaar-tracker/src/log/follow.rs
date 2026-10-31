//! Following a growing `Player.log`: start at the end, wait for the file if it is missing, and
//! notice when the game truncates or replaces it.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// How often the file is looked at.
pub const POLL_INTERVAL: Duration = Duration::from_millis(500);
/// The most bytes taken from the file in one pass.
pub const MAX_PASS_BYTES: u64 = 16 * 1024 * 1024;
/// How many bytes before our position are remembered to tell that the file is still the same one.
pub const TAIL_SIGNATURE_LEN: usize = 256;

/// Follows one log file. Call [`LogFollower::poll`] now and then; it returns the complete lines
/// written since the last call.
pub struct LogFollower {
    path: PathBuf,
    /// Where the next pass reads from, or `None` until the file has been seen for the first time.
    position: Option<u64>,
    /// The bytes just before `position`.
    tail: Vec<u8>,
    /// Whether the file was missing when we looked. A file that appears after that is new, so it is
    /// read from its first line, not its end.
    saw_missing: bool,
}

impl LogFollower {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into(), position: None, tail: Vec::new(), saw_missing: false }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// One pass. The first time the file exists this only notes where its end is, so an old session
    /// is never replayed. A missing file gives no lines and is waited for. Lines are returned
    /// without their line ending; blank lines are dropped.
    pub fn poll(&mut self) -> io::Result<Vec<String>> {
        let mut file = match File::open(&self.path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.saw_missing = true;
                return Ok(Vec::new());
            }
            Err(error) => return Err(error),
        };
        let length = file.metadata()?.len();

        let mut position = match self.position {
            Some(position) => position,
            None if self.saw_missing => 0,
            None => {
                self.start_at(&mut file, length)?;
                return Ok(Vec::new());
            }
        };

        // A shorter file, or one whose tail is not what we read, is a new file: read it from the top.
        if length < position || !tail_matches(&mut file, position, &self.tail) {
            position = 0;
            self.tail.clear();
        }
        if length == position {
            self.position = Some(position);
            return Ok(Vec::new());
        }

        let wanted = (length - position).min(MAX_PASS_BYTES) as usize;
        file.seek(SeekFrom::Start(position))?;
        let mut buffer = vec![0; wanted];
        file.read_exact(&mut buffer)?;

        // Only whole lines: a partial last line waits for the next pass.
        let Some(last_newline) = buffer.iter().rposition(|&byte| byte == b'\n') else {
            self.position = Some(position);
            return Ok(Vec::new());
        };
        let consumed = &buffer[..=last_newline];
        self.position = Some(position + consumed.len() as u64);
        self.remember_tail(consumed);

        let text = String::from_utf8_lossy(consumed);
        Ok(text.split('\n').map(|line| line.trim_end_matches('\r')).filter(|line| !line.trim().is_empty()).map(str::to_owned).collect())
    }

    fn start_at(&mut self, file: &mut File, end: u64) -> io::Result<()> {
        let from = end.saturating_sub(TAIL_SIGNATURE_LEN as u64);
        file.seek(SeekFrom::Start(from))?;
        self.tail = vec![0; (end - from) as usize];
        file.read_exact(&mut self.tail)?;
        self.position = Some(end);
        Ok(())
    }

    /// Keep the last [`TAIL_SIGNATURE_LEN`] bytes of everything consumed so far.
    fn remember_tail(&mut self, consumed: &[u8]) {
        self.tail.extend_from_slice(consumed);
        if self.tail.len() > TAIL_SIGNATURE_LEN {
            self.tail.drain(..self.tail.len() - TAIL_SIGNATURE_LEN);
        }
    }

    /// Poll every [`POLL_INTERVAL`] until `stop` is set, giving each pass's lines to `on_lines`.
    /// A read error is passed to `on_error` and the next pass tries again.
    pub fn run(&mut self, stop: &AtomicBool, mut on_lines: impl FnMut(Vec<String>), mut on_error: impl FnMut(io::Error)) {
        while !stop.load(Ordering::Relaxed) {
            match self.poll() {
                Ok(lines) if lines.is_empty() => {}
                Ok(lines) => on_lines(lines),
                Err(error) => on_error(error),
            }
            // Sleep in short steps so a stop request is noticed quickly.
            let mut slept = Duration::ZERO;
            while slept < POLL_INTERVAL && !stop.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(25));
                slept += Duration::from_millis(25);
            }
        }
    }
}

/// Whether the bytes just before `position` are still `tail`. A file that cannot be read for this
/// counts as matching, so a passing error does not throw away our place.
fn tail_matches(file: &mut File, position: u64, tail: &[u8]) -> bool {
    if tail.is_empty() {
        return true;
    }
    let mut now = vec![0; tail.len()];
    let Some(from) = position.checked_sub(tail.len() as u64) else { return false };
    if file.seek(SeekFrom::Start(from)).is_err() || file.read_exact(&mut now).is_err() {
        return true;
    }
    now == tail
}

#[cfg(test)]
#[path = "follow_tests.rs"]
mod tests;
