use std::path::{Path, PathBuf};

use log::info;
use mime_guess::Mime;

pub struct FileServer {
    base_path: PathBuf,
}

pub struct ServedFile {
    pub data: Vec<u8>,
    pub mime_type: Mime,
}

impl FileServer {
    pub fn new<P: Into<PathBuf>>(base_path: P) -> Self {
        Self {
            base_path: base_path.into(),
        }
    }

    pub async fn serve(&self, path: &Path) -> anyhow::Result<ServedFile> {
        let path = self.base_path.join(path);
        let path = path_clean::clean(path);
        info!("Try serving file {path:?}");
        let file = tokio::fs::read(&path).await.map(|content| {
            let content_guess = mime_guess::from_path(&path).first_or(mime_guess::mime::TEXT_PLAIN);
            info!("Serving file {path:?} of type {content_guess}");
            ServedFile {
                data: content,
                mime_type: content_guess,
            }
        })?;
        Ok(file)
    }
}
