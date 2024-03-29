use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};

use chrono::{DateTime, Utc};
use log::info;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use yaml_front_matter::YamlFrontMatter;

#[derive(Serialize, Deserialize, Clone)]
pub struct PostMetadata {
    pub title: String,
    pub author: String,
    pub publish_date: DateTime<Utc>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct BlogInfo {
    pub name: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct BlogEntry {
    pub description: PostMetadata,
    pub html: String,
    pub creation_date: SystemTime,
    pub filename: String,
}

pub struct BlogStorage {
    base_path: PathBuf,

    entries: RwLock<HashMap<String, Arc<BlogEntry>>>,
    most_recent_entries: RwLock<Vec<Arc<BlogEntry>>>,
    max_most_recent_entries: usize,
}

impl BlogStorage {
    pub fn new<P: AsRef<Path>>(base: P) -> Self {
        Self {
            base_path: PathBuf::from(base.as_ref()),
            entries: Default::default(),
            most_recent_entries: Default::default(),
            max_most_recent_entries: 10,
        }
    }

    pub async fn get_entry(&self, entry_name: &str) -> anyhow::Result<Arc<BlogEntry>> {
        if let Some(cached_entry) = self.try_find_cached_entry(entry_name).await {
            info!("Hit a cache entry for {entry_name}");
            Ok(cached_entry)
        } else {
            info!("Entry {entry_name} not found in cache, attempting to load it");
            let entry = Self::parse_file_to_html(&self.base_path.join(entry_name)).await?;
            let entry = Arc::new(entry);
            self.try_store_entry(entry_name, entry.clone()).await;
            Ok(entry)
        }
    }

    pub async fn remove_entry(&self, entry_name: String) {
        self.entries.write().await.remove_entry(&entry_name);
    }

    pub async fn try_store_entry(&self, entry_name: &str, entry: Arc<BlogEntry>) {
        let old = self
            .entries
            .write()
            .await
            .insert(entry_name.to_owned(), entry.clone());
        info!("Entry {entry_name} successfully stored in cache");
        if old.is_some() {
            // Avoid inserting again entry
            return;
        }

        let mut entries = self.most_recent_entries.write().await;

        if entries.iter().any(|e| e.filename == entry.filename) {
            // Avoid inserting again entry
            // Another check just to be extra sure
            return;
        }
        match entries.binary_search_by(|e| {
            entry
                .description
                .publish_date
                .cmp(&e.description.publish_date)
        }) {
            Ok(pos) => entries.insert(pos, entry),
            Err(pos) => entries.insert(pos, entry),
        }

        entries.truncate(self.max_most_recent_entries);
    }

    pub async fn contains_entry(&self, entry_name: &str) -> bool {
        self.entries
            .read()
            .await
            .keys()
            .any(|e| e.as_str() == entry_name)
    }

    pub async fn iterate_most_recent_entries<F: FnMut(&BlogEntry)>(&self, mut f: F) {
        self.most_recent_entries
            .read()
            .await
            .iter()
            .for_each(|entry| f(entry));
    }

    pub async fn parse_file_to_html<P: AsRef<Path>>(path: &P) -> anyhow::Result<BlogEntry> {
        let content = tokio::fs::read_to_string(&path).await?;
        let meta = tokio::fs::metadata(path).await?;
        let document = YamlFrontMatter::parse::<PostMetadata>(&content);
        let document = match document {
            Ok(doc) => doc,
            Err(e) => {
                anyhow::bail!(e.to_string())
            }
        };
        let html = comrak::markdown_to_html(&document.content, &comrak::Options::default());
        let filename = path.as_ref().to_path_buf();
        let filename = filename.file_name().unwrap().to_string_lossy();
        let filename = filename.to_string();
        Ok(BlogEntry {
            description: document.metadata,
            html,
            creation_date: meta.created()?,
            filename,
        })
    }

    async fn try_find_cached_entry(&self, entry_name: &str) -> Option<Arc<BlogEntry>> {
        self.entries.read().await.get(entry_name).cloned()
    }
}
