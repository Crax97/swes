use std::{
    collections::HashMap,
    fs::FileType,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use log::{error, info, warn};
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
        let mut entries = Default::default();

        Self::add_most_recent_entries(&mut entries, 10, &base)
            .expect("Failed to add most recent entries");

        Self {
            base_path: PathBuf::from(base.as_ref()),
            entries: RwLock::new(entries),
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

    fn add_most_recent_entries(
        entries: &mut HashMap<String, Arc<BlogEntry>>,
        max_entries: usize,
        base_path: &impl AsRef<Path>,
    ) -> anyhow::Result<()> {
        let entries_iterator = std::fs::read_dir(base_path)?
            .filter_map(|e| e.ok())
            .filter(|e| match e.file_type() {
                Ok(t) => t.is_file(),
                Err(_) => false,
            }); // for now ignore the max entries param

        for entry in entries_iterator {
            let entry_name = entry.file_name();
            let entry_name = entry_name.to_string_lossy();
            let entry_name = entry_name.to_string();

            let blog_entry = tokio::task::block_in_place(move || {
                tokio::runtime::Handle::current()
                    .block_on(async move { Self::parse_file_to_html(&entry.path()).await })
            });
            let blog_entry = match blog_entry {
                Ok(e) => e,
                Err(e) => {
                    warn!("Failed to read blog entry {}", e);
                    continue;
                }
            };

            info!("Added entry {}", entry_name);
            entries.insert(entry_name, Arc::new(blog_entry));
        }

        Ok(())
    }

    fn try_store_entry(&self, entry_name: &str, entry: Arc<BlogEntry>) {
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
