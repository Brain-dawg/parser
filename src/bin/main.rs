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
    filename: String,
    header: Header,
    #[serde(flatten)]
    state: MatchState,
}

#[derive(Serialize, Deserialize)]
struct Data {
    data: Vec<JsonDemo>,
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

    let mut output_file = File::create("output.json")?;
    let mut data = Vec::new();

    for entry in entries {
        let path = entry.unwrap().path();
        if path.is_file() && path.extension().unwrap_or_default() == "zip" {
            let file = fs::File::open(&path).unwrap();
            let mut archive = ZipArchive::new(file).unwrap();

            for i in 0..archive.len() {
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
                let json_demo = JsonDemo { filename: path.file_name().unwrap().to_str().unwrap().to_string(), header, state };

                let file_stem = path.file_stem().and_then(|s| s.to_str());
                println!("Writing {}", file_stem.unwrap_or("Unknown"));

                data.push(json_demo);

                fs::remove_file(&dem_path)?;
            }
        }
    }

    let data = Data { data };
    write!(output_file, "{}\n", serde_json::to_string(&data)?)?;

    Ok(())
}

fn main() -> Result<(), MainError> {
    loop {
        let _ = batchparse();
        thread::sleep(Duration::from_secs(5));
    }
}
