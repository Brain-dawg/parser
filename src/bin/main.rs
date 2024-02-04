use std::env;
use std::fs;

use main_error::MainError;
use serde::{Deserialize, Serialize};
use tf_demo_parser::demo::header::Header;
use tf_demo_parser::demo::parser::analyser::MatchState;
pub use tf_demo_parser::{Demo, DemoParser, Parse, ParseError, ParserState, Stream};

#[cfg(feature = "jemallocator")]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonDemo {
    header: HeaderWithFilename,
    #[serde(flatten)]
    state: MatchState,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HeaderWithFilename {
    filename: String,
    #[serde(flatten)]
    header: Header,
}

use std::io::Write;
use std::io::Read;
use std::fs::File;
use zip::ZipArchive;
use std::thread;
use std::time::Duration;

fn batchparse() -> Result<(), MainError> {
    #[cfg(feature = "better_panic")]
    better_panic::install();

    #[cfg(feature = "trace")]
    tracing_subscriber::fmt::init();

    let args: Vec<_> = env::args().collect();
    let all = args.contains(&std::string::String::from("all"));

    let entries = fs::read_dir(".").unwrap();

    // Create a new JSON file to write the output
    let mut output_file = File::create("all_demos.json")?;

    // Start the JSON array
    write!(output_file, "[")?;

    let mut first = true;

    for entry in entries {
        let path = match entry {
            Ok(entry) => entry.path(),
            Err(_) => continue,
        };
    
        if path.is_file() && path.extension().unwrap_or_default() == "zip" {
            let file = match fs::File::open(&path) {
                Ok(file) => file,
                Err(_) => {
                    println!("Failed to open file: {:?}", path);
                    continue;
                }
            };
            let mut archive = match ZipArchive::new(file) {
                Ok(archive) => archive,
                Err(_) => {
                    println!("Failed to read archive: {:?}", path);
                    continue;
                }
            };
    
            for i in 0..archive.len() {
                if path.with_extension("json").exists() { continue; }
    
                let mut zip_file = match archive.by_index(i) {
                    Ok(file) => file,
                    Err(_) => {
                        println!("Failed to read file from archive: {:?}", path);
                        continue;
                    }
                };
                let mut contents = Vec::new();
                if let Err(_) = zip_file.read_to_end(&mut contents) {
                    println!("Failed to read contents of file from archive: {:?}", path);
                    continue;
                }
    
                let dem_path = path.with_extension("dem");
                let mut dem_file = match fs::File::create(&dem_path) {
                    Ok(file) => file,
                    Err(_) => {
                        println!("Failed to create .dem file: {:?}", dem_path);
                        continue;
                    }
                };
                if let Err(_) = dem_file.write_all(&contents) {
                    println!("Failed to write to .dem file: {:?}", dem_path);
                    continue;
                }
    
                let demo_file_contents = match fs::read(&dem_path) {
                    Ok(contents) => contents,
                    Err(_) => {
                        println!("Failed to read .dem file: {:?}", dem_path);
                        continue;
                    }
                };
                let demo = Demo::new(&demo_file_contents);
    
                let parser = if all {
                    DemoParser::new_all(demo.get_stream())
                } else {
                    DemoParser::new(demo.get_stream())
                };
                let (header, state) = match parser.parse() {
                    Ok((header, state)) => (header, state),
                    Err(_) => {
                        println!("Failed to parse demo: {:?}", dem_path);
                        continue;
                    }
                };
                let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Unknown");
                let json_demo = JsonDemo { header: HeaderWithFilename { filename: file_stem.to_string(), header }, state };
    
                // Skip the current iteration if the filename field is present in the header
                if json_demo.header.filename.is_some() {
                    continue;
                }
    
                if !first {
                    if let Err(_) = write!(output_file, ",") {
                        println!("Failed to write to output file");
                        continue;
                    }
                }
    
                if let Err(_) = write!(output_file, "{{\"data\": {}}}", serde_json::to_string(&json_demo).unwrap()) {
                    println!("Failed to write JSON to output file");
                    continue;
                }
    
                first = false;
    
                if let Err(_) = fs::remove_file(&dem_path) {
                    println!("Failed to remove .dem file: {:?}", dem_path);
                }
            }
        }
    }    

    // End the JSON array
    write!(output_file, "]")?;

    Ok(())
}

fn main() -> Result<(), MainError> {
    loop {
        let _ = batchparse();
        thread::sleep(Duration::from_secs(5));
    }
}
