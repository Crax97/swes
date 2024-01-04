use std::path::{Path, PathBuf};

use handlebars::Handlebars;
use serde::Serialize;

use crate::blog_storage::{BlogEntry, BlogInfo};

const BLOG_ENTRY: &str = "blog_entry";
const BLOG_ENTRY_NOT_FOUND: &str = "entry_not_found";
const HOME: &str = "home";

fn load_handlebars_theme<P: AsRef<Path>>(path: P) -> anyhow::Result<Handlebars<'static>> {
    const BLOG_ENTRY_FILE: &str = "blog_entry.handlebars";
    const BLOG_ENTRY_NOT_FOUND_FILE: &str = "entry_not_found.handlebars";
    const HOME_FILE: &str = "home.handlebars";
    let mut handlebars = Handlebars::new();
    handlebars.register_template_string(
        BLOG_ENTRY,
        std::fs::read_to_string(path.as_ref().join(BLOG_ENTRY_FILE))?,
    )?;

    handlebars.register_template_string(
        BLOG_ENTRY_NOT_FOUND,
        std::fs::read_to_string(path.as_ref().join(BLOG_ENTRY_NOT_FOUND_FILE))?,
    )?;

    handlebars.register_template_string(
        HOME,
        std::fs::read_to_string(path.as_ref().join(HOME_FILE))?,
    )?;
    Ok(handlebars)
}

pub struct HandlebarsSupport {
    handlebars: Handlebars<'static>,
    theme_path: PathBuf,
}

#[derive(Serialize)]
struct HomeContent {
    blog_info: BlogInfo,
    important_entries: Vec<BlogEntry>,
}

impl HandlebarsSupport {
    pub fn new<P: AsRef<Path>>(theme_path: P) -> anyhow::Result<Self> {
        let handlebars = load_handlebars_theme(&theme_path)?;
        Ok(Self {
            handlebars,
            theme_path: theme_path.as_ref().to_path_buf(),
        })
    }

    pub fn reload_theme(&mut self) -> anyhow::Result<()> {
        let handlebars = load_handlebars_theme(&self.theme_path)?;
        self.handlebars = handlebars;
        Ok(())
    }

    pub fn format_blog_entry(&self, blog_entry: &BlogEntry) -> String {
        self.handlebars.render(BLOG_ENTRY, blog_entry).unwrap()
    }

    pub fn format_home(&self, blog_info: BlogInfo, important_entries: Vec<BlogEntry>) -> String {
        let home_info = HomeContent {
            blog_info,
            important_entries,
        };
        self.handlebars.render(HOME, &home_info).unwrap()
    }

    pub fn format_not_found(&self, entry: String) -> String {
        self.handlebars
            .render(BLOG_ENTRY_NOT_FOUND, &entry)
            .unwrap()
    }
}
