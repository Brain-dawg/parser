use std::env;
use std::fs;
use std::io;
use std::io::Write;
use std::io::Read;
// use std::io::Seek;
use std::fs::File;
use zip::write::FileOptions;
use zip::CompressionMethod;
// use flate2::Compression;
use std::collections::HashSet;
use std::fs::remove_file;
use std::path::PathBuf;
// use std::thread;
use std::time::SystemTime;
use main_error::MainError;
use serde::{Deserialize, Serialize};
use tf_demo_parser::demo::header::Header;
use tf_demo_parser::demo::parser::analyser::MatchState;
use tf_demo_parser::{Demo, DemoParser};

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

fn batchparse(processed_files: &mut HashSet<String>) -> Result<(), MainError> {
    let args: Vec<_> = env::args().collect();
    let all = args.contains(&std::string::String::from("all"));
    let entries = fs::read_dir(".")?;
    let file_path = "all_demos_new.json";
    let mut output_file = File::create(file_path)?;
    write!(output_file, "[")?;

    for entry in entries {
        let path = match entry {
            Ok(entry) => entry.path(),
            Err(_) => continue,
        };

        if path.is_file() && path.extension().unwrap_or_default() == "zip" {
            let file = match fs::File::open(&path) {
                Ok(file) => file,
                Err(_) => continue,
            };
            let mut archive = match zip::ZipArchive::new(file) {
                Ok(archive) => archive,
                Err(_) => continue,
            };

            for i in 0..archive.len() {
                if path.with_extension("json").exists() {
                    continue;
                }

                let mut zip_file = match archive.by_index(i) {
                    Ok(file) => file,
                    Err(_) => continue,
                };

                let mut contents = Vec::new();
                if let Err(_) = zip_file.read_to_end(&mut contents) {
                    continue;
                }

                let dem_path = path.with_extension("dem");
                let mut dem_file = match fs::File::create(&dem_path) {
                    Ok(file) => file,
                    Err(_) => continue,
                };
                if let Err(_) = dem_file.write_all(&contents) {
                    continue;
                }

                let demo_file_contents = match fs::read(&dem_path) {
                    Ok(contents) => contents,
                    Err(_) => continue,
                };
                let demo = Demo::new(&demo_file_contents);

                let parser = if all {
                    DemoParser::new_all(demo.get_stream())
                } else {
                    DemoParser::new(demo.get_stream())
                };
                let (header, state) = match parser.parse() {
                    Ok((header, state)) => (header, state),
                    Err(_) => continue,
                };
                let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Unknown");

                let json_demo = JsonDemo { header: HeaderWithFilename { filename: file_stem.to_string(), header }, state };

                if !processed_files.contains(&json_demo.header.filename) {
                    if processed_files.is_empty() {
                        write!(output_file, "{}", serde_json::to_string(&json_demo)?)?;
                    } else {
                        write!(output_file, ",{}", serde_json::to_string(&json_demo)?)?;
                    }
                    println!("Writing: {:?}", &json_demo.header.filename)
                }

                processed_files.insert(json_demo.header.filename);

                if let Err(_) = remove_file(&dem_path) {
                    continue;
                }
            }
        }
    }

    write!(output_file, "]")?;

    // Generate timestamp
    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    // Specify the directory where the zip file will be saved
    let parsed_old_dir = PathBuf::from("parsed_old");
    fs::create_dir_all(&parsed_old_dir)?; // Create the directory if it doesn't exist

    // Compose the full path for the zip file
    let zip_filename = parsed_old_dir.join(format!("all_demos_{}.zip", timestamp));

    // let zip_path = Path::new(&zip_filename);

    let options = FileOptions::default().compression_method(CompressionMethod::Deflated);

    let zip_file = fs::File::create(&zip_filename)?;
    let mut zip_writer = zip::ZipWriter::new(zip_file);
    zip_writer.start_file("all_demos.json", options)?;
    
    let mut all_demos = fs::File::open("all_demos.json")?;
    io::copy(&mut all_demos, &mut zip_writer)?;
    zip_writer.finish()?;
    // Close the zip file
    drop(zip_writer);

    // Remove the old JSON file
    fs::remove_file("all_demos.json")?;

    // Overwrite old JSON file with new data
    fs::rename(file_path, "all_demos.json")?;

    Ok(())
}

fn main() -> Result<(), MainError> {
    let mut processed_files = HashSet::new();
    batchparse(&mut processed_files)?;
    Ok(())
}
