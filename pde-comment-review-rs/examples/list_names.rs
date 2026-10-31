//! Prints each named comment in the given files as `file:line name`,
//! to compare with the names today's comment checker lists.

use std::env;
use std::fs;
use std::path::Path;

use pde_comment_review::shared::comments::find_comments;

fn main() {
    for file in env::args().skip(1) {
        let source = match fs::read_to_string(&file) {
            Ok(source) => source,
            Err(error) => {
                eprintln!("{file}: {error}");
                continue;
            }
        };
        match find_comments(Path::new(&file), &source) {
            Ok(comments) => {
                for comment in comments {
                    if let Some(name) = comment.name {
                        println!("{file}:{} {name}", comment.line);
                    }
                }
            }
            Err(error) => eprintln!("{file}: {error}"),
        }
    }
}
