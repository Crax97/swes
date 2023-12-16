use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::RwLock,
};

use log::error;
use serde::{Deserialize, Serialize};
use yaml_front_matter::YamlFrontMatter;

#[derive(Serialize, Deserialize, Clone)]
pub struct PostMetadata {
    pub title: String,
}

#[derive(Clone)]
pub struct BlogEntry {
    pub description: PostMetadata,
    pub html: String,
}

pub struct BlogStorage {
    base_path: PathBuf,

    entries: RwLock<HashMap<String, BlogEntry>>,
}

impl BlogStorage {
    pub fn new<P: AsRef<Path>>(base: P) -> Self {
        Self {
            base_path: PathBuf::from(base.as_ref()),
            entries: RwLock::default(),
        }
    }

    pub async fn get_entry(&self, entry_name: &str) -> anyhow::Result<BlogEntry> {
        if let Some(cached_entry) = self.try_find_cached_entry(entry_name) {
            Ok(cached_entry)
        } else {
            let entry = Self::parse_file_to_html(&self.base_path.join(entry_name)).await?;
            self.try_store_entry(entry_name, &entry);
            Ok(entry)
        }
    }

    fn try_store_entry(&self, entry_name: &str, entry: &BlogEntry) {
        match self.entries.write() {
            Ok(mut storage) => {
                storage.insert(entry_name.to_owned(), entry.clone());
            }
            Err(e) => {
                error!("Poised entry storage on write: {e}");
            }
        }
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

    fn try_find_cached_entry(&self, entry_name: &str) -> Option<BlogEntry> {
        match self.entries.read() {
            Ok(entries) => entries.get(entry_name).cloned(),
            Err(e) => {
                error!("Poised entry storage on read: {e}");
                None
            }
        }
    }
}
