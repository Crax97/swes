mod blog_storage;

use clap::Parser;

use crate::blog_storage::BlogStorage;

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long)]
    file: String,
    #[arg(short, long)]
    base_path: Option<String>,
}

fn main() {
    let args = Args::parse();
    let storage = BlogStorage::new(args.base_path.unwrap_or("blog".to_owned()));
    let post = storage.get_entry(&args.file).unwrap();
    println!("Title: {}", post.description.title);
    println!("Content: \n{}", post.html);
}
