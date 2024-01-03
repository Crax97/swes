mod blog_storage;

use std::{convert::Infallible, sync::Arc};

use clap::Parser;
use log::info;
use warp::Filter;

use crate::blog_storage::BlogStorage;

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long)]
    file: Option<String>,
    #[arg(short, long)]
    base_path: Option<String>,
}

#[tokio::main]
async fn main() {
    env_logger::init();
    let args = Args::parse();
    let storage = BlogStorage::new(args.base_path.unwrap_or("blog".to_owned()));
    let storage = Arc::new(storage);

    let routes = warp::path!("blog" / String).and_then({
        let storage = storage.clone();

        move |entry| {
            let storage = storage.clone();
            async move { Ok::<_, Infallible>(blog(entry, storage).await) }
        }
    });
    warp::serve(routes).run(([127, 0, 0, 1], 8080)).await;
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
