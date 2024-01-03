use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use log::{error, info};
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

    entries: RwLock<HashMap<String, Arc<BlogEntry>>>,
}

impl BlogStorage {
    pub fn new<P: AsRef<Path>>(base: P) -> Self {
        Self {
            base_path: PathBuf::from(base.as_ref()),
            entries: Default::default(),
        }
    }

    pub async fn get_entry(&self, entry_name: &str) -> anyhow::Result<Arc<BlogEntry>> {
        if let Some(cached_entry) = self.try_find_cached_entry(entry_name) {
            info!("Hit a cache entry for {entry_name}");
            Ok(cached_entry)
        } else {
            info!("Entry {entry_name} not found in cache, attempting to load it");
            let entry = Self::parse_file_to_html(&self.base_path.join(entry_name)).await?;
            let entry = Arc::new(entry);
            self.try_store_entry(entry_name, entry.clone());
            Ok(entry)
        }
    }

    pub async fn remove_entry(&self, entry_name: String) {
        match self.entries.write() {
            Ok(mut h) => {
                h.remove_entry(&entry_name);
            }
            Err(e) => {
                error!("Failed to remove entry {entry_name}: {e}");
            }
        }
    }

    pub fn try_store_entry(&self, entry_name: &str, entry: Arc<BlogEntry>) {
        match self.entries.write() {
            Ok(mut storage) => {
                info!("Entry {entry_name} successfully stored in cache");
                storage.insert(entry_name.to_owned(), entry);
            }
            Err(e) => {
                error!("Poised entry storage on write: {e}");
            }
        }
    }

    pub async fn contains_entry(&self, entry_name: &str) -> bool {
        match self.entries.read() {
            Ok(e) => e.keys().any(|e| e.as_str() == entry_name),
            Err(_) => false,
        }
    }

    pub async fn parse_file_to_html<P: AsRef<Path>>(path: &P) -> anyhow::Result<BlogEntry> {
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

    fn try_find_cached_entry(&self, entry_name: &str) -> Option<Arc<BlogEntry>> {
        match self.entries.read() {
            Ok(entries) => entries.get(entry_name).cloned(),
            Err(e) => {
                error!("Poised entry storage on read: {e}");
                None
            }
        }
    }
}
