use std::path::PathBuf;

/// Lets every held engine go when the test ends, passed or failed.
pub struct LetGo(pub PathBuf);

impl Drop for LetGo {
    fn drop(&mut self) {
        let _ = std::fs::write(&self.0, "");
    }
}
