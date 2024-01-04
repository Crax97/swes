use std::path::{Path, PathBuf};

use handlebars::Handlebars;
use serde::Serialize;

use crate::blog_storage::{BlogEntry, BlogInfo};

const BLOG_ENTRY: &str = "blog_entry";
const BLOG_ENTRY_NOT_FOUND: &str = "entry_not_found";
const HOME: &str = "home";

const HANDLEBARS_RELOAD_SCRIPT: &str = include_str!("../static/hot_reload.js");
const HANDLEBARS_RELOAD_PARTIAL: &str = "hot_reload_script";

fn load_handlebars_theme<P: AsRef<Path>>(path: P) -> anyhow::Result<Handlebars<'static>> {
    const BLOG_ENTRY_FILE: &str = "blog_entry.handlebars";
    const BLOG_ENTRY_NOT_FOUND_FILE: &str = "entry_not_found.handlebars";
    const HOME_FILE: &str = "home.handlebars";

    let mut handlebars = Handlebars::new();
    handlebars.register_partial(HANDLEBARS_RELOAD_PARTIAL, HANDLEBARS_RELOAD_SCRIPT)?;
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

#[derive(Serialize)]
struct BlogContent {
    blog_info: BlogInfo,
    blog_entry: BlogEntry,
}

#[derive(Serialize)]
struct NotFoundContent {
    blog_info: BlogInfo,
    entry_not_found: String,
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

    pub fn format_blog_entry(&self, blog_info: BlogInfo, blog_entry: &BlogEntry) -> String {
        let entry_info = BlogContent {
            blog_info,
            blog_entry: blog_entry.clone(),
        };
        self.handlebars.render(BLOG_ENTRY, &entry_info).unwrap()
    }

    pub fn format_home(&self, blog_info: BlogInfo, important_entries: Vec<BlogEntry>) -> String {
        let home_info = HomeContent {
            blog_info,
            important_entries,
        };
        self.handlebars.render(HOME, &home_info).unwrap()
    }

    pub fn format_not_found(&self, blog_info: BlogInfo, entry_not_found: String) -> String {
        let entry_info = NotFoundContent {
            blog_info,
            entry_not_found,
        };
        self.handlebars
            .render(BLOG_ENTRY_NOT_FOUND, &entry_info)
            .unwrap()
    }
}
