use halo_core::media_input::MediaInput;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// `MediaInput` implementation for local filesystem paths (desktop + Android local files).
pub struct LocalFsInput(BufReader<File>);

impl LocalFsInput {
    pub fn open(path: &Path) -> Result<Box<dyn MediaInput>, String> {
        let file = File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
        Ok(Box::new(Self(BufReader::new(file))))
    }
}

impl std::io::Read for LocalFsInput {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

impl std::io::Seek for LocalFsInput {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        self.0.seek(pos)
    }
}

impl MediaInput for LocalFsInput {}
