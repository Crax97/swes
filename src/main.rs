use serde::{Deserialize, Serialize};
use std::path::Path;

use clap::Parser;
use yaml_front_matter::YamlFrontMatter;

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long)]
    file: String,
}

#[derive(Serialize, Deserialize)]
struct Frontmatter {
    title: String,
}

struct ParsedDocument {
    description: Frontmatter,
    html: String,
}

fn main() {
    let args = Args::parse();
    let file = parse_file_to_html(&args.file).unwrap();
    println!("Title: {}", file.description.title);
    println!("Content: \n{}", file.html);
}

fn parse_file_to_html<P: AsRef<Path>>(
    path: &P,
) -> Result<ParsedDocument, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let document = YamlFrontMatter::parse::<Frontmatter>(&content)?;
    let html = comrak::markdown_to_html(&document.content, &comrak::Options::default());
    Ok(ParsedDocument {
        description: document.metadata,
        html,
    })
}
