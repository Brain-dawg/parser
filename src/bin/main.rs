use std::env;
use std::fs;

use main_error::MainError;
use serde::{Deserialize, Serialize};
use tf_demo_parser::demo::header::Header;
use tf_demo_parser::demo::parser::analyser::MatchState;
// use tf_demo_parser::demo::parser::player_summary_analyzer::PlayerSummaryAnalyzer;
pub use tf_demo_parser::{Demo, DemoParser, Parse, ParseError, ParserState, Stream};

#[cfg(feature = "jemallocator")]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonDemo {
    header: Header,
    #[serde(flatten)]
    state: MatchState,
}

use std::io::Write;
use std::io::Read;
use std::fs::File;
use std::path::Path;
use zip::ZipArchive;
use std::thread;
use std::time::Duration;
use serde_json::json;

fn batchparse() -> Result<(), MainError> {
    #[cfg(feature = "better_panic")]
    better_panic::install();

    #[cfg(feature = "trace")]
    tracing_subscriber::fmt::init();

    let args: Vec<_> = env::args().collect();
    //args removed, just parse everything in the current directory
    // if args.len() < 2 {
    //     println!("1 argument required");
    //     return Ok(());
    // }
    // let path = args[1].clone();
    let all = args.contains(&std::string::String::from("all"));
    // let detailed_summaries = args.contains(&std::string::String::from("detailed_summaries")); //removed
    // let file = fs::read(path)?;
    // let demo = Demo::new(&file);
    // Get an iterator over the entries in the current directory
    let mut filenames = Vec::new();
    let entries = fs::read_dir(".").unwrap();
    for entry in entries {
        // Get the path of the entry
        let path = entry.unwrap().path();

        // Check if the path is a file and has a .zip extension
        if path.is_file() && path.extension().unwrap_or_default() == "zip" {
            // Open the file as a ZipArchive
            let file = fs::File::open(&path).unwrap();
            let mut archive = ZipArchive::new(file).unwrap();

            // Loop through each file in the archive
            for i in 0..archive.len() {

                // Get the filename and store it in the vector
                if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                    let filename = filename.to_string();
                    let filename = filename.replace(".zip", ".json");
                    filenames.push(filename);
                }

                if path.with_extension("json").exists() { continue; }

                // Get the ZipFile by index
                let mut zip_file = archive.by_index(i).unwrap();

                // Create a Vec<u8> to store the contents
                let mut contents = Vec::new();

                // Read the contents to the Vec<u8>
                zip_file.read_to_end(&mut contents).unwrap();

                // Create a .dem file with the same name as the zip file
                let dem_path = path.with_extension("dem");
                let mut dem_file = fs::File::create(&dem_path).unwrap();

                // Write the contents to the .dem file
                dem_file.write_all(&contents).unwrap();

                // Create a new JSON file to write the output
                let mut output_file = File::create(path.with_extension("json"))?;

                // Parse the demo file
                let demo_file_contents = fs::read(&dem_path)?;
                let demo = Demo::new(&demo_file_contents);

                let parser = if all {
                    DemoParser::new_all(demo.get_stream())
                } else {
                    DemoParser::new(demo.get_stream())
                };
                let (header, state) = parser.parse()?;
                let json_demo = JsonDemo { header, state };

                let file_stem = path.file_stem().and_then(|s| s.to_str());
                println!("Writing JSON for {}", file_stem.unwrap_or("Unknown"));

                // Write the JSON output to the file
                write!(output_file, "[{}]", serde_json::to_string(&json_demo)?)?;

                // File written, delete extracted demo file
                fs::remove_file(&dem_path)?;
            }
        }
    }
    let parsed_file = Path::new("parsed.json");
    let mut file = File::create(&parsed_file)?;
    file.write_all(json!(&filenames).to_string().as_bytes())?;
    Ok(())
}

fn main() -> Result<(), MainError> {
    loop {
        let _ = batchparse();
        thread::sleep(Duration::from_secs(5));
    }
}
