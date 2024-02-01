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
        let path = entry.unwrap().path();

        if path.is_file() && path.extension().unwrap_or_default() == "zip" {
            let file = fs::File::open(&path).unwrap();
            let mut archive = ZipArchive::new(file).unwrap();

            for i in 0..archive.len() {
                if path.with_extension("json").exists() { continue; }

                let mut zip_file = archive.by_index(i).unwrap();
                let mut contents = Vec::new();
                zip_file.read_to_end(&mut contents).unwrap();

                let dem_path = path.with_extension("dem");
                let mut dem_file = fs::File::create(&dem_path).unwrap();
                dem_file.write_all(&contents).unwrap();

                let demo_file_contents = fs::read(&dem_path)?;
                let demo = Demo::new(&demo_file_contents);

                let parser = if all {
                    DemoParser::new_all(demo.get_stream())
                } else {
                    DemoParser::new(demo.get_stream())
                };
                let (header, state) = parser.parse()?;
                let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Unknown");
                let json_demo = JsonDemo { header: HeaderWithFilename { filename: file_stem.to_string(), header }, state };

                // If this is not the first demo, add a comma to separate the JSON objects
                if !first {
                    write!(output_file, ",")?;
                }

                // Write the JSON output to the file, excluding the filename
                write!(output_file, "{{\"data\": {}}}", serde_json::to_string(&json_demo)?)?;

                first = false;

                fs::remove_file(&dem_path)?;
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
