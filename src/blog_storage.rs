use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use yaml_front_matter::YamlFrontMatter;

#[derive(Serialize, Deserialize)]
pub struct PostMetadata {
    pub title: String,
}

pub struct BlogEntry {
    pub description: PostMetadata,
    pub html: String,
}

pub struct BlogStorage {
    base_path: PathBuf,
}

impl BlogStorage {
    pub fn new<P: AsRef<Path>>(base: P) -> Self {
        Self {
            base_path: PathBuf::from(base.as_ref()),
        }
    }

    pub async fn get_entry(&self, entry_name: &str) -> anyhow::Result<BlogEntry> {
        Self::parse_file_to_html(&self.base_path.join(entry_name)).await
    }

    async fn parse_file_to_html<P: AsRef<Path>>(path: &P) -> anyhow::Result<BlogEntry> {
        let content = tokio::fs::read_to_string(&path).await?;
        let document = YamlFrontMatter::parse::<PostMetadata>(&content);
        let document = match document {
            Ok(doc) => doc,
            Err(e) => {
                anyhow::bail!(e.to_string())
            }
        };
        let html = comrak::markdown_to_html(&document.content, &comrak::Options::default());
        Ok(BlogEntry {
            description: document.metadata,
            html,
        })
    }
}
