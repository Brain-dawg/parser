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
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
// use std::thread;
use std::time::SystemTime;
use main_error::MainError;
use serde::{Deserialize, Serialize};
use tf_demo_parser::demo::header::Header;
use tf_demo_parser::demo::parser::analyser::MatchState;
use tf_demo_parser::{Demo, DemoParser};
use mysql::*;
use mysql::prelude::*;
// Import regex crate
use regex::Regex;


const DB_TABLE_NAME: &str = "demo_data";

fn db_connect() -> Result<Pool, Box<dyn std::error::Error>> {

    // Database connection constants
    const DB_HOST: &str = "";
    const DB_PORT: u16 = ;
    const DB_USER: &str = "";
    const DB_PASS: &str = "";
    const DB_NAME: &str = "";

    let opts = OptsBuilder::new()
        .ip_or_hostname(Some(DB_HOST))
        .tcp_port(DB_PORT)
        .user(Some(DB_USER))
        .pass(Some(DB_PASS))
        .db_name(Some(DB_NAME));

    let pool = Pool::new(opts)?;

    Ok(pool)
}


#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonDemo {
    header: HeaderWithFilename,
    #[serde(flatten)]
    state: MatchState,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonDemoNoNesting {
    filename: String,
    header: Header,
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

fn batchparse_database() -> Result<(), MainError> {

    let pool = db_connect()?;

    let args: Vec<_> = env::args().collect();
    let all = args.contains(&std::string::String::from("all"));
    let entries = fs::read_dir(".")?;

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
                let filename = file_stem.to_string();

                let json_demo = JsonDemoNoNesting { filename, header, state };

                // Create a HashMap to store the JSON data
                let mut json_map = std::collections::HashMap::new();

                // Insert the filename as the key and the JsonDemoNoNesting struct as the value
                json_map.insert(json_demo.filename.clone(), json_demo);

                // Convert the HashMap to a HashSet of JSON strings
                let json_set: std::collections::HashSet<String> = json_map
                    .into_iter()
                    .map(|(_, v)| serde_json::to_string(&v).unwrap_or_default())
                    .collect();

                // Print every key-value pair in the json_set
                for json_string in &json_set {

                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(json_string) {

                        if let Some(obj) = value.as_object() {

                            let mut query_params = Vec::new();
                            
                            let mut filename = String::new();

                            if obj.contains_key("filename") {
                                filename = obj.get("filename").unwrap().as_str().unwrap().to_string();
                            }

                            for (key, val) in obj {

                                if key == "header" {

                                    for (k, v) in val.as_object().unwrap() {
                                        
                                        query_params.push((k.clone(), v.clone()));
                                        // println!("Key: {}, Value: {}", k, v);
                                    }
                                }
                                else if key.to_lowercase() == "chat" {

                                    if let Some(arr) = val.as_array() {

                                        for chat_entry in arr {

                                            if let Some(obj) = chat_entry.as_object() {

                                                let mut chat_values = Vec::new();
                                                let columns = ["from", "kind", "text", "tick"];
                                                
                                                for &column in &columns {

                                                    let value = match obj.get(column) {

                                                        Some(serde_json::Value::String(s)) => Value::from(s.clone()),
                                                        
                                                        Some(serde_json::Value::Number(n)) if column == "tick" => {

                                                            if let Some(i) = n.as_i64() {
                                                                Value::from(i)
                                                            } else {
                                                                Value::from(0)
                                                            }
                                                        },
                                                        _ => Value::NULL,
                                                    };
                                                    chat_values.push(value);
                                                }

                                                chat_values.push(Value::from(filename.clone()));

                                                let chat_query = format!(
                                                    "INSERT INTO demo_chat (`from`, `kind`, `text`, `tick`, `filename`) 
                                                    SELECT ?, ?, ?, ?, ?
                                                    WHERE NOT EXISTS (
                                                        SELECT 1 FROM demo_chat 
                                                        WHERE `from` = ? AND `kind` = ? AND `text` = ? AND `tick` = ? AND `filename` = ?
                                                    )"
                                                );

                                                // Duplicate the values for the WHERE clause
                                                let mut query_values = chat_values.clone();
                                                query_values.extend_from_slice(&chat_values);

                                                // Get a connection from the pool
                                                if let Ok(mut conn) = pool.get_conn() {
                                                    // Execute the query
                                                    match conn.exec_drop(chat_query, query_values) {
                                                        Ok(_) => println!("Successfully processed chat data for file: {}", filename),
                                                        Err(e) => eprintln!("Error processing chat data for file {}: {}", filename, e),
                                                    }
                                                } else {
                                                    eprintln!("Failed to get database connection for chat data insertion");
                                                }
                                            }
                                        }
                                    }
                                }
                                else if key == "filename" {
                                    filename = val.as_str().unwrap_or_default().to_string();
                                    query_params.push((key.clone(), val.clone()));
                                }
                            }

                            // Add the filename to the values
                            let mut values: Vec<Value> = Vec::new();
                            let mut columns: Vec<String> = Vec::new();

                            query_params.iter().for_each(|(key, v)| {
                                match v {
                                    serde_json::Value::String(s) => {
                                        if key == "server" {
                                            let re = Regex::new(r"Server #(\d+) \[(\w+)\]").unwrap();
                                            if let Some(caps) = re.captures(s) {
                                                let server_id = caps.get(1).map_or("", |m| m.as_str());
                                                let region = caps.get(2).map_or("", |m| m.as_str());

                                                columns.push("server_id".to_string());
                                                columns.push("region".to_string());
                                                values.push(Value::from(server_id));
                                                values.push(Value::from(region));
                                            }
                                            columns.push(key.clone());
                                            values.push(Value::from(s.clone()));
                                        } else {
                                            columns.push(key.clone());
                                            values.push(Value::from(s.clone()));
                                        }
                                    }
                                    serde_json::Value::Number(n) => {
                                        columns.push(key.clone());
                                        if let Some(i) = n.as_i64() {
                                            values.push(Value::from(i));
                                        } else if let Some(f) = n.as_f64() {
                                            values.push(Value::from(f));
                                        }
                                    },
                                    serde_json::Value::Bool(b) => {
                                        columns.push(key.clone());
                                        values.push(Value::from(*b));
                                    },
                                    _ => {
                                        columns.push(key.clone());
                                        values.push(Value::NULL);
                                    },
                                }
                            });

                            // Add the filename to the columns and values
                            // columns.push("filename".to_string());
                            // values.push(Value::from(filename.clone()));

                            let placeholders: Vec<String> = (1..=columns.len()).map(|_| "?".to_string()).collect();

                            // println!("Placeholder length: {}, columns length: {}", placeholders.len(), columns.len());
                            // println!("Values: {:?}\n, Columns: {:?}", values, columns);

                            let query = format!(
                                "INSERT INTO `{}` ({}) 
                                SELECT {}
                                WHERE NOT EXISTS (
                                    SELECT 1 FROM `{}`
                                    WHERE `filename` = '{}'
                                )",
                                DB_TABLE_NAME,
                                columns.join(", "),
                                placeholders.join(", "),
                                DB_TABLE_NAME,
                                filename.replace("'", "''") // Escape single quotes in the filename
                            );

                            // Get a connection from the pool
                            let mut conn = pool.get_conn()?;

                            // Execute the query
                            match conn.exec_drop(query, values) {
                                Ok(_) => println!("Successfully inserted data for file: {}", filename),
                                Err(e) => eprintln!("Error inserting data for file {}: {}", filename, e),
                            }
                        }
                    }
                }
                // let json_data = serde_json::to_string(&json_set)?;
                // println!("Adding to hashmap: {:?}", &json_data);

                // processed_files.insert(json_demo.header.filename);

                if let Err(_) = fs::remove_file(&dem_path) {
                    continue;
                }
            }
        }
    }
    Ok(())
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
                // let filename = file_stem.to_string();

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

                if let Err(_) = fs::remove_file(&dem_path) {
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
    if let Err(err_) = fs::remove_file("all_demos.json")
    {
        println!("{:?}", err_)
    }

    // Overwrite old JSON file with new data
    fs::rename(file_path, "all_demos.json")?;

    Ok(())
}

fn main() -> Result<(), MainError> {
    let new_dir = Path::new("D:/parser/target/release/demos");
    env::set_current_dir(&new_dir).unwrap();

    let path = Path::new("all_demos.json");
    if !path.exists() {
        OpenOptions::new().write(true).create_new(true).open(path)?;
    }

    // let mut processed_files = HashSet::new();
    // batchparse(&mut processed_files)?;
    batchparse_database()?;
    Ok(())
}

