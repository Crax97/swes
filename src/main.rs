mod blog_storage;

use std::{convert::Infallible, sync::Arc};

use clap::Parser;
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
    let entry = storage.get_entry(&entry).await;
    if let Ok(entry) = entry {
        entry.html.clone()
    } else {
        "<h1>Not found</h1>".to_owned()
    }
}
