mod blog_storage;

use std::{
    convert::Infallible,
    path::{Path, PathBuf},
    sync::Arc,
};

use clap::Parser;
use log::{error, info, warn};
use notify::{
    event::{CreateKind, DataChange, ModifyKind, RemoveKind, RenameMode},
    RecursiveMode, Watcher,
};
use tokio::runtime::Handle;
use warp::Filter;

use crate::blog_storage::BlogStorage;

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long)]
    file: Option<String>,
    #[arg(short, long)]
    base_path: Option<String>,
}
fn create_entry(p: PathBuf, storage: Arc<BlogStorage>, handle: Handle) {
    handle.spawn(async move {
        let blog_entry = BlogStorage::parse_file_to_html(&p).await;
        let entry_name = p.file_name().unwrap();
        let entry_name = entry_name.to_string_lossy();
        if !is_valid_filename_entry(&entry_name) {
            info!("Ignoring entry {entry_name} for insertion");
            return;
        }
        let blog_entry = match blog_entry {
            Ok(e) => e,
            Err(e) => {
                error!("Failed to read entry {entry_name}: {e}");
                return;
            }
        };
        info!("Storing new entry {entry_name}");
        storage.try_store_entry(&entry_name, Arc::new(blog_entry));
    });
}

fn reload_entry(path: PathBuf, watcher_storage: Arc<BlogStorage>, handle: Handle) {
    handle.spawn(async move {
        let filename = path.file_name();
        let entry_name = if let Some(filename) = filename {
            filename.to_string_lossy().to_string()
        } else {
            return;
        };
        if !is_valid_filename_entry(&entry_name) {
            info!("Ignoring entry {entry_name} for reload");
            return;
        }
        if watcher_storage.contains_entry(&entry_name).await {
            {
                info!("Reloading entry {entry_name}");
                let blog_entry = BlogStorage::parse_file_to_html(&path).await;
                let entry_name = path.file_name().unwrap();
                let entry_name = entry_name.to_string_lossy();
                let blog_entry = match blog_entry {
                    Ok(e) => e,
                    Err(e) => {
                        error!("Failed to read entry {entry_name}: {e}");
                        return;
                    }
                };
                watcher_storage.try_store_entry(&entry_name, Arc::new(blog_entry));
            }
        }
    });
}

fn is_valid_filename_entry(filename: &str) -> bool {
    filename.ends_with(".md") && !filename.starts_with('_')
}

fn remove_entry(path: PathBuf, watcher_storage: Arc<BlogStorage>, handle: Handle) {
    handle.spawn(async move {
        let filename = path.file_name();
        let filename = if let Some(filename) = filename {
            filename.to_string_lossy().to_string()
        } else {
            return;
        };
        if !filename.ends_with(".md") {
            info!("Ignoring file removal {path:?}");
            return;
        }
        info!("Removing entry {filename}");
        watcher_storage.remove_entry(filename.to_owned()).await;
    });
}

fn add_most_recent_entries(
    storage: &mut BlogStorage,
    _max_entries: usize,
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
                .block_on(async move { BlogStorage::parse_file_to_html(&entry.path()).await })
        });
        let blog_entry = match blog_entry {
            Ok(e) => e,
            Err(e) => {
                warn!("Failed to read blog entry {}", e);
                continue;
            }
        };

        if !is_valid_filename_entry(&entry_name) {
            info!("Ignoring entry {entry_name}");
            continue;
        }
        info!("Added entry {}", entry_name);
        storage.try_store_entry(&entry_name, Arc::new(blog_entry));
    }

    Ok(())
}
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let args = Args::parse();

    let base_path = args.base_path.unwrap_or("blog".to_owned());

    let mut storage = BlogStorage::new(base_path.clone());
    add_most_recent_entries(&mut storage, 10, &base_path)?;
    let storage = Arc::new(storage);

    let watcher_storage = storage.clone();
    let handle = tokio::runtime::Handle::current();

    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        match res {
            Ok(evt) => match evt.kind {
                notify::EventKind::Create(f) if f == CreateKind::File => create_entry(
                    evt.paths[0].clone(),
                    watcher_storage.clone(),
                    handle.clone(),
                ),
                notify::EventKind::Modify(f)
                    if matches!(
                        f,
                        ModifyKind::Name(RenameMode::To)
                            | ModifyKind::Data(DataChange::Any | DataChange::Content)
                    ) =>
                {
                    reload_entry(
                        evt.paths[0].clone(),
                        watcher_storage.clone(),
                        handle.clone(),
                    );
                }
                notify::EventKind::Remove(f) if f == RemoveKind::File => remove_entry(
                    evt.paths[0].clone(),
                    watcher_storage.clone(),
                    handle.clone(),
                ),
                _ => {}
            },
            Err(e) => println!("err {e:?}"),
        };
    })
    .expect("watcher");
    watcher.watch(Path::new(&base_path), RecursiveMode::NonRecursive)?;

    let routes = warp::path!("blog" / String).and_then({
        let storage = storage.clone();

        move |entry| {
            let storage = storage.clone();
            async move { Ok::<_, Infallible>(blog(entry, storage).await) }
        }
    });
    warp::serve(routes).run(([127, 0, 0, 1], 8080)).await;
    Ok(())
}

async fn blog(entry: String, storage: Arc<BlogStorage>) -> String {
    let entry_name = entry.clone();
    let entry = storage.get_entry(&entry).await;
    if let Ok(entry) = entry {
        info!("Serving entry {entry_name}");
        entry.html.clone()
    } else {
        info!("Entry {entry_name} not found");
        "<h1>Not found</h1>".to_owned()
    }
}
