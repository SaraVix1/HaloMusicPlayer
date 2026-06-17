/// Seekable byte stream used by the audio decoder.
///
/// Desktop: `LocalFsInput` wraps a `BufReader<File>`.
/// Android (Phase D): content URI via `ParcelFileDescriptor`.
/// iOS (Phase E): security-scoped bookmark export.
///
/// `Send + Sync` are required because rodio's `Decoder` needs them.
pub trait MediaInput: std::io::Read + std::io::Seek + Send + Sync {}
