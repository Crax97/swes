mod blog_storage;
mod file_server;
mod handlebars_support;

use futures_util::StreamExt;
use std::{
    convert::Infallible,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use blog_storage::BlogInfo;
use clap::Parser;
use file_server::FileServer;
use handlebars_support::HandlebarsSupport;
use log::{error, info, warn};
use notify::{
    event::{CreateKind, DataChange, ModifyKind, RemoveKind, RenameMode},
    RecursiveMode, Watcher,
};
use tokio::{
    runtime::Handle,
    sync::broadcast::{Receiver, Sender},
};
use warp::{
    filters::sse::Event,
    reply::{Html, Reply, Response},
    Filter,
};

use crate::blog_storage::BlogStorage;

fn blog_info() -> BlogInfo {
    BlogInfo {
        name: "Crax's blog".to_owned(),
    }
}

#[derive(Clone)]
pub enum UpdateEvent {
    Reload,
}

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long)]
    base_path: Option<String>,

    #[arg(short, long)]
    file_server_path: Option<String>,

    #[arg(long)]
    handlebars_theme: Option<String>,

    #[arg(long)]
    address: Option<String>,

    #[arg(long)]
    port: Option<u16>,
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
    let file_path = args.file_server_path.unwrap_or("files".to_owned());
    let handlebars_theme = args.handlebars_theme.unwrap_or("default".to_owned());

    let handlebars_path = Path::new("themes").join(handlebars_theme);

    let mut storage = BlogStorage::new(base_path.clone());
    add_most_recent_entries(&mut storage, 10, &base_path)?;
    let storage = Arc::new(storage);

    let file_server = FileServer::new(file_path);
    let file_server = Arc::new(file_server);

    let handlebars_support = HandlebarsSupport::new(&handlebars_path)?;
    let handlebars_support = Arc::new(RwLock::new(handlebars_support));

    let watcher_storage = storage.clone();
    let handle = tokio::runtime::Handle::current();

    let (send, _): (Sender<UpdateEvent>, Receiver<UpdateEvent>) =
        tokio::sync::broadcast::channel(500);

    let md_sender = send.clone();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        match res {
            Ok(evt) => match evt.kind {
                notify::EventKind::Create(f) if f == CreateKind::File => {
                    create_entry(
                        evt.paths[0].clone(),
                        watcher_storage.clone(),
                        handle.clone(),
                    );
                    let _ = md_sender.send(UpdateEvent::Reload);
                }
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
                    let _ = md_sender.send(UpdateEvent::Reload);
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

    let handlebars_support_watcher = handlebars_support.clone();
    let handlebars_sender = send.clone();
    let mut handlebars_watcher =
        notify::recommended_watcher(move |res: notify::Result<notify::Event>| match res {
            Ok(evt) => {
                if let notify::EventKind::Modify(_) = evt.kind {
                    info!("Reloading handlebars theme");
                    handlebars_support_watcher
                        .write()
                        .expect("Failed to write hb support")
                        .reload_theme()
                        .unwrap_or_else(|e| error!("Handlebars reload failed: {e}"));
                    let _ = handlebars_sender.send(UpdateEvent::Reload);
                }
            }
            Err(e) => error!("err {e:?}"),
        })
        .expect("handlebars watcher");
    handlebars_watcher.watch(&handlebars_path, RecursiveMode::NonRecursive)?;
    handlebars_watcher.watch(Path::new("files/style.css"), RecursiveMode::NonRecursive)?;

    let blog = warp::path!("blog" / String).and_then({
        let storage = storage.clone();

        let handlebars_support = handlebars_support.clone();
        move |entry| {
            let storage = storage.clone();
            let handlebars_support = handlebars_support.clone();
            async move { Ok::<_, Infallible>(blog(entry, storage, handlebars_support).await) }
        }
    });
    let home = warp::path!("blog").and_then({
        let storage = storage.clone();
        let handlebars_support = handlebars_support.clone();

        move || {
            let storage = storage.clone();
            let handlebars_support = handlebars_support.clone();
            async move { Result::<_, Infallible>::Ok(home(storage, handlebars_support).await) }
        }
    });
    let files = warp::path!("files" / String).and_then(move |path| {
        let file_server = file_server.clone();
        async move { Ok::<_, Infallible>(file(PathBuf::from(path), file_server.clone()).await) }
    });
    let events = warp::path!("events").and(warp::get()).map(move || {
        let receiver = send.subscribe();
        sse_update(receiver)
    });
    info!("Serve ready");

    let addr = args.address.unwrap_or("127.0.0.1".to_owned());
    let port = args.port.unwrap_or(8080);
    warp::serve(blog.or(home).or(files).or(events))
        .run(SocketAddr::new(addr.parse().unwrap(), port))
        .await;
    Ok(())
}

async fn blog(
    entry: String,
    storage: Arc<BlogStorage>,
    handlebars_support: Arc<RwLock<HandlebarsSupport>>,
) -> Html<String> {
    let entry_name = entry.clone();
    let entry = storage.get_entry(&entry).await;
    let handlebars_support = handlebars_support
        .read()
        .expect("Failed to open handlebars support");
    if let Ok(entry) = entry {
        info!("Serving entry {entry_name}");
        warp::reply::html(handlebars_support.format_blog_entry(blog_info(), &entry))
    } else {
        info!("Entry {entry_name} not found");
        warp::reply::html(handlebars_support.format_not_found(blog_info(), entry_name))
    }
}

async fn home(
    storage: Arc<BlogStorage>,
    handlebars_support: Arc<RwLock<HandlebarsSupport>>,
) -> Html<String> {
    let mut accum = Vec::new();
    storage.iterate_most_recent_entries(|e| accum.push(e.clone()));
    let home = handlebars_support
        .read()
        .expect("Poised handlebars support")
        .format_home(blog_info(), accum);
    warp::reply::html(home)
}

async fn file(path: PathBuf, file_server: Arc<FileServer>) -> Response {
    match file_server.serve(&path).await {
        Ok(file) => warp::reply::with_header(file.data, "content-type", file.mime_type.to_string())
            .into_response(),
        Err(e) => {
            error!("While serving request {path:?} error '{e}' happened");
            warp::reply::with_status(
                warp::reply::html("<h1>Not found</h1>"),
                warp::http::StatusCode::NOT_FOUND,
            )
            .into_response()
        }
    }
}

fn sse_data(evt: UpdateEvent) -> Result<Event, Infallible> {
    let event = match evt {
        UpdateEvent::Reload => "reload",
    };
    Ok(Event::default().data(event.to_string()))
}

fn sse_update(receiver: Receiver<UpdateEvent>) -> impl Reply {
    let stream = tokio_stream::wrappers::BroadcastStream::new(receiver);

    let stream = stream.map(move |event| match event {
        Ok(event) => sse_data(event),
        Err(e) => {
            error!("While receiving reload event: {e}");
            sse_data(UpdateEvent::Reload)
        }
    });
    warp::sse::reply(stream)
}
