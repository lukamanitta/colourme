use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::path::PathBuf;

extern crate dirs;

fn usage() {
    println!(
        "usage:
            colourme <string>"
    );
}

fn main() {
    let argv: Vec<String> = env::args().collect();
    let colourscheme_name: &String;
    match argv.len() {
        2 => {
            colourscheme_name = &argv[1];
        }
        _ => {
            usage();
            return;
        }
    };

    let mut colourme_dir = dirs::home_dir().unwrap();
    colourme_dir.push(".config/colourme");

    // Get config file and construct Config struct

    let mut colourscheme_path: PathBuf = dirs::home_dir().unwrap();
    colourscheme_path.push(format!(".config/colourme/schemes/{colourscheme_name}.toml"));
    let colourscheme_path_display = colourscheme_path.display();

    let mut colourscheme_file = match File::open(&colourscheme_path) {
        Err(why) => {
            println!("ERROR: Couldn't open {colourscheme_path_display}: {why}");
            return;
        }
        Ok(colourscheme_file) => colourscheme_file,
    };

    let mut colourscheme_content = String::new();
    match colourscheme_file.read_to_string(&mut colourscheme_content) {
        Err(why) => {
            println!("Couldn't read {colourscheme_path_display}: {why}");
            return;
        }
        Ok(_) => print!("{colourscheme_path_display} contains: \n{colourscheme_content}"),
    };

    // Convert to Colourscheme object, validate that at least base16 colours defined

    // Parse config file to find all available templates & destinations
    // For each template-destination
    //      Open template file
    //      Copy to /tmp/ file
    //      Find every double-curly-bracket section
    //      Evaluate expression after format identifier (<format>:<expr>)
    //          Can be simple variable substitution or function eval
    //          Raise errors if expr fails
    //      Serialise Color object to specified format
    //          Raise error if format is invalid
    //      Replace double-curly-bracket section with serialised colour
    //      Copy from /tmp/ to destination from config
    //      Delete /tmp/ file
}
