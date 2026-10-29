use std::{
    fs::File,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

pub struct Writer {
    path: PathBuf,
    file: Option<BufWriter<File>>,
}

impl Writer {
    pub fn new(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let file = File::create(&path)?; // ??open lazily on first write -> but still truncate even if no tokens found
        Ok(Self {
            path,
            file: Some(BufWriter::new(file)),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn write_header(&mut self, path_name: &str) -> std::io::Result<()> {
        self.write(&format!("======================\n{path_name}\n"))
    }

    pub fn write_entry(&mut self, lines_count: usize, line: &str) -> std::io::Result<()> {
        self.write(&format!("\tline {lines_count}: {line}"))
    }

    fn write(&mut self, text: &str) -> std::io::Result<()> {
        if let Some(file) = self.file.as_mut() {
            file.write_all(text.as_bytes())?;
            file.flush()?;
        }
        Ok(())
    }

    pub fn close(&mut self) -> std::io::Result<()> {
        if let Some(mut file) = self.file.take() {
            file.flush()?;
        }
        Ok(())
    }
}
