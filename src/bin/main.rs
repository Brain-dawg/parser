/*
 * TODO
 * =========
 * - Skip JSON serialization for database
 *     - This was copied over from the original function + easier to implement
 */


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
use dotenv::dotenv;
use md5;

// #[cfg(target_os = "linux")]
// const IS_LINUX: bool = true;

// #[cfg(not(target_os = "linux"))]
// const IS_LINUX: bool = false;

const DB_TABLE_NAME: &str = "demo_data";
const DB_CHAT_TABLE_NAME: &str = "demo_chat";

fn db_connect() -> Result<Pool, Box<dyn std::error::Error>> {
    let current_dir = env::current_dir().expect("Failed to get current directory");
    println!("Current directory: {:?}", current_dir);

    let env_path = current_dir.join("..").join("..").join(".env");
    println!("Attempting to read .env file from: {:?}", env_path);

    let env_content = match fs::read_to_string(&env_path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Failed to read .env file: {}", e);
            eprintln!("Searched for .env file at: {:?}", env_path);
            eprintln!("Current directory contents:");
            if let Ok(entries) = fs::read_dir(current_dir) {
                for entry in entries {
                    if let Ok(entry) = entry {
                        println!("  {:?}", entry.path());
                    }
                }
            }
            return Err(Box::new(e));
        }
    };
    
    let db_host = env_content.lines()
        .find(|line| line.starts_with("DB_HOST="))
        .and_then(|line| line.split('=').nth(1))
        .expect("DB_HOST not found in .env file");
    
    let db_port: u16 = env_content.lines()
        .find(|line| line.starts_with("DB_PORT="))
        .and_then(|line| line.split('=').nth(1))
        .and_then(|port| port.parse().ok())
        .expect("DB_PORT not found or invalid in .env file");
    
    let db_user = env_content.lines()
        .find(|line| line.starts_with("DB_USER="))
        .and_then(|line| line.split('=').nth(1))
        .expect("DB_USER not found in .env file");
    
    let db_pass = env_content.lines()
        .find(|line| line.starts_with("DB_PASS="))
        .and_then(|line| line.split('=').nth(1))
        .expect("DB_PASS not found in .env file");
    
    let db_name = env_content.lines()
        .find(|line| line.starts_with("DB_NAME="))
        .and_then(|line| line.split('=').nth(1))
        .expect("DB_NAME not found in .env file");

    let opts = OptsBuilder::new()
        .ip_or_hostname(Some(db_host))
        .tcp_port(db_port)
        .user(Some(db_user))
        .pass(Some(db_pass))
        .db_name(Some(db_name));

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

#[derive(Debug)]
struct NoJsonDemo {
    filename: String,
    header: Header,
    state: MatchState,
}


#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HeaderWithFilename {
    filename: String,
    #[serde(flatten)]
    header: Header,
}

// fn batchparse_database() -> Result<(), MainError> {

//     let pool = db_connect()?;

//     let args: Vec<_> = env::args().collect();
//     let all = args.contains(&std::string::String::from("all"));
//     let entries = fs::read_dir(".")?;

//     for entry in entries {
//         let path = match entry {
//             Ok(entry) => entry.path(),
//             Err(_) => continue,
//         };

//         if path.is_file() && path.extension().unwrap_or_default() == "zip" {
//             let file = match fs::File::open(&path) {
//                 Ok(file) => file,
//                 Err(_) => continue,
//             };
//             let mut archive = match zip::ZipArchive::new(file) {
//                 Ok(archive) => archive,
//                 Err(_) => continue,
//             };

//             for i in 0..archive.len() {
//                 if path.with_extension("json").exists() {
//                     continue;
//                 }

//                 let mut zip_file = match archive.by_index(i) {
//                     Ok(file) => file,
//                     Err(_) => continue,
//                 };

//                 let mut contents = Vec::new();
//                 if let Err(_) = zip_file.read_to_end(&mut contents) {
//                     continue;
//                 }

//                 let dem_path = path.with_extension("dem");
//                 let mut dem_file = match fs::File::create(&dem_path) {
//                     Ok(file) => file,
//                     Err(_) => continue,
//                 };
//                 if let Err(_) = dem_file.write_all(&contents) {
//                     continue;
//                 }

//                 let demo_file_contents = match fs::read(&dem_path) {
//                     Ok(contents) => contents,
//                     Err(_) => continue,
//                 };
//                 let demo = Demo::new(&demo_file_contents);

//                 let parser = if all {
//                     DemoParser::new_all(demo.get_stream())
//                 } else {
//                     DemoParser::new(demo.get_stream())
//                 };
//                 let (header, state) = match parser.parse() {
//                     Ok((header, state)) => (header, state),
//                     Err(_) => continue,
//                 };
//                 let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Unknown");
//                 let filename = file_stem.to_string();

//                 let json_demo = JsonDemoNoNesting { filename, header, state };

//                 // Create a HashMap to store the JSON data
//                 let mut json_map = std::collections::HashMap::new();

//                 // Insert the filename as the key and the JsonDemoNoNesting struct as the value
//                 json_map.insert(json_demo.filename.clone(), json_demo);

//                 // Convert the HashMap to a HashSet of JSON strings
//                 let json_set: std::collections::HashSet<String> = json_map
//                     .into_iter()
//                     .map(|(_, v)| serde_json::to_string(&v).unwrap_or_default())
//                     .collect();

//                 // Print every key-value pair in the json_set
//                 for json_string in &json_set {

//                     if let Ok(value) = serde_json::from_str::<serde_json::Value>(json_string) {

//                         if let Some(obj) = value.as_object() {

//                             let mut query_params = Vec::new();
                            
//                             let mut filename = String::new();
//                             // Generate filename_hash from the complete filename
//                             let filename_hash = if obj.contains_key("filename") {
//                                 let fname = obj.get("filename")
//                                     .and_then(|v| v.as_str())
//                                     .unwrap_or("unknown")
//                                     .to_string();
//                                 filename = fname.clone();
//                                 // Ensure we're using the complete filename for hashing
//                                 let digest = md5::compute(fname.as_bytes());
//                                 let hash = format!("{:x}", digest);
//                                 // println!("Generated MD5 hash for '{}': {}", fname, hash);  // Debug print
//                                 hash
//                             } else {
//                                 let digest = md5::compute(b"unknown");
//                                 let hash = format!("{:x}", digest);
//                                 println!("Using default MD5 hash: {}", hash);  // Debug print
//                                 hash
//                             };

//                             // Debug prints to verify the complete data
//                             // println!("Full filename: {}", filename);
//                             // println!("Computed MD5 Hash: {}", filename_hash);

//                             // query_params.push(("filename_hash".to_string(), serde_json::Value::String(filename_hash.clone())));

//                             for (key, val) in obj {

//                                 if key == "header" {

//                                     for (k, v) in val.as_object().unwrap() {
                                        
//                                         query_params.push((k.clone(), v.clone()));
//                                         // println!("Key: {}, Value: {}", k, v);
//                                     }
//                                 }
//                                 else if key.to_lowercase() == "chat" {

//                                     if let Some(arr) = val.as_array() {

//                                         for chat_entry in arr {

//                                             if let Some(obj) = chat_entry.as_object() {

//                                                 let mut chat_values = Vec::new();
//                                                 let columns = ["tick", "from", "kind", "text", "filename", "filename_hash", "text_hash"];
//                                                 let placeholders: Vec<String> = (1..=columns.len()).map(|_| "?".to_string()).collect();
                                                
//                                                 let mut text_hash = String::new();

//                                                 if let Some(text) = obj.get("text").and_then(|v| v.as_str()) {
//                                                     text_hash = format!("{:x}", md5::compute(text));
//                                                 }

//                                                 for &column in columns.iter() {
//                                                     let value = match column {
//                                                         "filename" => Value::from(filename.clone()),
//                                                         "filename_hash" => Value::from(filename_hash.clone()),
//                                                         "text_hash" => Value::from(text_hash.clone()),
//                                                         _ => match obj.get(column) {
//                                                             Some(serde_json::Value::String(s)) => Value::from(s.clone()),
//                                                             Some(serde_json::Value::Number(n)) if column == "tick" => {
//                                                                 if let Some(i) = n.as_i64() {
//                                                                     Value::from(i)
//                                                                 } else {
//                                                                     Value::from(0)
//                                                                 }
//                                                             },
//                                                             _ => Value::NULL,
//                                                         },
//                                                     };
//                                                     chat_values.push(value);
//                                                 }

//                                                 let columns_str = columns.iter().map(|c| format!("`{}`", c)).collect::<Vec<String>>();
//                                                 let chat_query = format!(
//                                                     "INSERT IGNORE INTO {} ({}) 
//                                                     VALUES ({})",
//                                                     DB_CHAT_TABLE_NAME,
//                                                     columns_str.join(", "),
//                                                     placeholders.join(", ")
//                                                 );

//                                                 // let chat_values_copy = chat_values.clone();
//                                                 // chat_values.extend(chat_values_copy);

//                                                 // Get a connection from the pool
//                                                 if let Ok(mut conn) = pool.get_conn() {
//                                                     // Execute the query
//                                                     match conn.exec_drop(chat_query, chat_values.clone()) {
//                                                         Ok(_) => (),
//                                                         Err(e) => eprintln!("Error processing chat data for file {}: {}", filename, e),
//                                                     }
//                                                 } else {
//                                                     eprintln!("Failed to get database connection for chat data insertion");
//                                                 }
//                                             }
//                                         }
//                                     }
//                                 }

//                                 else if key == "filename" {
//                                     filename = val.as_str().unwrap_or_default().to_string();
//                                     query_params.push((key.clone(), val.clone()));
//                                 }
//                             }

//                             let mut values: Vec<Value> = Vec::new();
//                             let mut columns: Vec<String> = Vec::new();

//                             query_params.iter().for_each(|(key, v)| {
//                                 match v {
//                                     serde_json::Value::String(s) => {
//                                         if key == "server" {
//                                             let re = Regex::new(r"[Ss]erver #(\d+) \[(\w+)\]").unwrap();
//                                             if let Some(caps) = re.captures(s) {
//                                                 let server_id = caps.get(1).map_or("", |m| m.as_str());
//                                                 let region = caps.get(2).map_or("", |m| m.as_str());

//                                                 columns.push("server_id".to_string());
//                                                 columns.push("region".to_string());
//                                                 values.push(Value::from(server_id));
//                                                 values.push(Value::from(region));
//                                             }
//                                             columns.push(key.clone());
//                                             values.push(Value::from(s.clone()));
//                                         } else {
//                                             columns.push(key.clone());
//                                             values.push(Value::from(s.clone()));
//                                         }
//                                     }
//                                     serde_json::Value::Number(n) => {
//                                         columns.push(key.clone());
//                                         if let Some(i) = n.as_i64() {
//                                             values.push(Value::from(i));
//                                         } else if let Some(f) = n.as_f64() {
//                                             values.push(Value::from(f));
//                                         }
//                                     },
//                                     serde_json::Value::Bool(b) => {
//                                         columns.push(key.clone());
//                                         values.push(Value::from(*b));
//                                     },
//                                     _ => {
//                                         columns.push(key.clone());
//                                         values.push(Value::NULL);
//                                     },
//                                 }
//                             });
                            
//                             //append the filename hash to the end
//                             columns.push("filename_hash".to_string());
//                             values.push(Value::from(filename_hash.clone()));
//                             // Add the filename to the columns and values
//                             // columns.push("filename".to_string());
//                             // values.push(Value::from(filename.clone()));

//                             let columns_str = columns.iter().map(|c| format!("`{}`", c)).collect::<Vec<String>>();
//                             let placeholders: Vec<String> = (1..=columns.len()).map(|_| "?".to_string()).collect();

//                             let query = format!(
//                                 "INSERT IGNORE INTO `{}` ({}) 
//                                 VALUES ({})",
//                                 DB_TABLE_NAME,
//                                 columns_str.join(", "),
//                                 placeholders.join(", ")
//                             );

//                             // println!("Query: {}", query);  // Debug print
//                             // println!("Filename: {}", filename);  // Debug print
//                             // println!("Hash: {}", filename_hash);  // Debug print

//                             // Get a connection from the pool
//                             let mut conn = pool.get_conn()?;

//                             // Remove filename_hash from query_params if it exists
//                             // query_params.retain(|(key, _)| key != "filename_hash");

//                             // Ensure we're using Value::from with the complete hash string
//                             // values.push(Value::from(filename_hash.clone()));

//                             // Execute with explicit debug information
//                             match conn.exec_drop(query, values.clone()) {
//                                 Ok(_) => println!("Successfully inserted record for {} with hash {}", filename, filename_hash),
//                                 Err(e) => eprintln!("Error inserting {} (hash: {}): {}", filename, filename_hash, e),
//                             }
//                         }
//                     }
//                 }
//                 // let json_data = serde_json::to_string(&json_set)?;
//                 // println!("Adding to hashmap: {:?}", &json_data);

//                 // processed_files.insert(json_demo.header.filename);

//                 if let Err(_) = fs::remove_file(&dem_path) {
//                     continue;
//                 }
//             }
//         }
//     }
//     Ok(())
// }

// fn batchparse_database_nojson() -> Result<(), MainError> {

//     let pool = db_connect()?;

//     let args: Vec<_> = env::args().collect();
//     let all = args.contains(&std::string::String::from("all"));
//     let entries = fs::read_dir(".")?;

//     for entry in entries {

//         let path = match entry {
//             Ok(entry) => entry.path(),
//             Err(_) => continue,
//         };

//         if path.is_file() && path.extension().unwrap_or_default() == "zip" {
//             let file = match fs::File::open(&path) {
//                 Ok(file) => file,
//                 Err(_) => continue,
//             };
//             let mut archive = match zip::ZipArchive::new(file) {
//                 Ok(archive) => archive,
//                 Err(_) => continue,
//             };

//             for i in 0..archive.len() {
//                 if path.with_extension("json").exists() {
//                     continue;
//                 }

//                 let mut zip_file = match archive.by_index(i) {
//                     Ok(file) => file,
//                     Err(_) => continue,
//                 };

//                 let mut contents = Vec::new();
//                 if let Err(_) = zip_file.read_to_end(&mut contents) {
//                     continue;
//                 }

//                 let dem_path = path.with_extension("dem");
//                 let mut dem_file = match fs::File::create(&dem_path) {
//                     Ok(file) => file,
//                     Err(_) => continue,
//                 };
//                 if let Err(_) = dem_file.write_all(&contents) {
//                     continue;
//                 }

//                 let demo_file_contents = match fs::read(&dem_path) {
//                     Ok(contents) => contents,
//                     Err(_) => continue,
//                 };
//                 let demo = Demo::new(&demo_file_contents);

//                 let parser = if all {
//                     DemoParser::new_all(demo.get_stream())
//                 } else {
//                     DemoParser::new(demo.get_stream())
//                 };
//                 let (header, state) = match parser.parse() {
//                     Ok((header, state)) => (header, state),
//                     Err(_) => continue,
//                 };
//                 let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Unknown");
//                 let filename = file_stem.to_string();

//                 let demo_data_nojson = NoJsonDemo { filename, header, state };

//                 // Create a HashMap to store the struct
//                 let mut nojson_map = std::collections::HashMap::new();

//                 nojson_map.insert(demo_data_nojson.filename.clone(), demo_data_nojson);

//                 // Print every key-value pair in the json_set
//                 for (filename, data) in &nojson_map {
                    
//                     // println!("Key: {}", filename);
//                     // println!("Demo protocol: {}", data.header.protocol);
//                     // println!("Server name: {}", data.header.server);
//                     // println!("Map name: {}", data.header.map);
//                     // println!("Ticks: {}", data.header.ticks);
//                     // println!("Frames: {}", data.header.frames);
//                     // println!("Duration: {}", data.header.duration);
//                     // println!("Value: {:?}", data.header);
//                     // println!("Value: {:?}", json_string);

//                     let mut query_params = Vec::new();

//                     query_params.push(("demo_type".to_string(), data.header.demo_type.to_string()));
//                     query_params.push(("duration".to_string(), data.header.duration.to_string()));
//                     query_params.push(("frames".to_string(), data.header.frames.to_string()));
//                     query_params.push(("game".to_string(), data.header.game.to_string()));
//                     query_params.push(("map".to_string(), data.header.map.to_string()));
//                     query_params.push(("nick".to_string(), data.header.nick.to_string()));
//                     query_params.push(("protocol".to_string(), data.header.protocol.to_string()));
//                     query_params.push(("server".to_string(), data.header.server.to_string()));
//                     query_params.push(("signon".to_string(), data.header.signon.to_string()));
//                     query_params.push(("ticks".to_string(), data.header.ticks.to_string()));
//                     query_params.push(("version".to_string(), data.header.version.to_string()));
//                     query_params.push(("filename".to_string(), filename.clone()));
                    
//                     // Generate filename_hash from the complete filename
//                     let filename_hash_digest = md5::compute(filename.as_bytes());
//                     let filename_hash = format!("{:x}", filename_hash_digest);

//                     // Debug prints to verify the complete data
//                     // println!("Full filename: {}", filename);
//                     // println!("Computed MD5 Hash: {}", filename_hash);

//                     // query_params.push(("filename_hash".to_string(), serde_json::Value::String(filename_hash.clone())));

                                
//                     query_params.push(("filename_hash".to_string(), filename_hash.clone()));
//                     // println!("Key: {}, Value: {}", k, v);
                            

//                     let arr = data.state.chat;

//                     for chat_entry in arr {

//                         let mut chat_values = Vec::new();
//                         let columns = ["tick", "from", "kind", "text", "filename", "filename_hash", "text_hash"];
//                         let placeholders: Vec<String> = (1..=columns.len()).map(|_| "?".to_string()).collect();

//                         let text_hash = format!("{:x}", md5::compute(chat_entry.text.as_str()));

//                             for &column in columns.iter() {
//                                 let value = match column {
//                                     "filename" => Value::from(filename),
//                                     "filename_hash" => Value::from(filename_hash),
//                                     "text_hash" => Value::from(text_hash),
//                                     _ => match chat_entry.get(column) {
//                                         Some(serde_json::Value::String(s)) => Value::from(s.clone()),
//                                         Some(serde_json::Value::Number(n)) if column == "tick" => {
//                                             if let Some(i) = n.as_i64() {
//                                                 Value::from(i)
//                                             } else {
//                                                 Value::from(0)
//                                             }
//                                         },
//                                         _ => Value::NULL,
//                                     },
//                                 };
//                                 chat_values.push(value);
//                             }

//                             let columns_str = columns.iter().map(|c| format!("`{}`", c)).collect::<Vec<String>>();
//                             let chat_query = format!(
//                                 "INSERT IGNORE INTO {} ({}) 
//                                 VALUES ({})",
//                                 DB_CHAT_TABLE_NAME,
//                                 columns_str.join(", "),
//                                 placeholders.join(", ")
//                             );

//                             // let chat_values_copy = chat_values.clone();
//                             // chat_values.extend(chat_values_copy);

//                             // Get a connection from the pool
//                             if let Ok(mut conn) = pool.get_conn() {
//                                 // Execute the query
//                                 match conn.exec_drop(chat_query, chat_values.clone()) {
//                                     Ok(_) => (),
//                                     Err(e) => eprintln!("Error processing chat data for file {}: {}", filename, e),
//                                 }
//                             } else {
//                                 eprintln!("Failed to get database connection for chat data insertion");
//                             }
//                         }
//                     }
//                 }

//                 let mut values: Vec<Value> = Vec::new();
//                 let mut columns: Vec<String> = Vec::new();

//                 query_params.iter().for_each(|(key, v)| {
//                     match v {
//                         serde_json::Value::String(s) => {
//                             if key == "server" {
//                                 let re = Regex::new(r"[Ss]erver #(\d+) \[(\w+)\]").unwrap();
//                                 if let Some(caps) = re.captures(s) {
//                                     let server_id = caps.get(1).map_or("", |m| m.as_str());
//                                     let region = caps.get(2).map_or("", |m| m.as_str());

//                                     columns.push("server_id".to_string());
//                                     columns.push("region".to_string());
//                                     values.push(Value::from(server_id));
//                                     values.push(Value::from(region));
//                                 }
//                                 columns.push(key.clone());
//                                 values.push(Value::from(s.clone()));
//                             } else {
//                                 columns.push(key.clone());
//                                 values.push(Value::from(s.clone()));
//                             }
//                         }
//                         serde_json::Value::Number(n) => {
//                             columns.push(key.clone());
//                             if let Some(i) = n.as_i64() {
//                                 values.push(Value::from(i));
//                             } else if let Some(f) = n.as_f64() {
//                                 values.push(Value::from(f));
//                             }
//                         },
//                         serde_json::Value::Bool(b) => {
//                             columns.push(key.clone());
//                             values.push(Value::from(*b));
//                         },
//                         _ => {
//                             columns.push(key.clone());
//                             values.push(Value::NULL);
//                         },
//                     }
//                 });
                
//                 //append the filename hash to the end
//                 columns.push("filename_hash".to_string());
//                 values.push(Value::from(filename_hash.clone()));
//                 // Add the filename to the columns and values
//                 // columns.push("filename".to_string());
//                 // values.push(Value::from(filename.clone()));

//                 let columns_str = columns.iter().map(|c| format!("`{}`", c)).collect::<Vec<String>>();
//                 let placeholders: Vec<String> = (1..=columns.len()).map(|_| "?".to_string()).collect();

//                 let query = format!(
//                     "INSERT IGNORE INTO `{}` ({}) 
//                     VALUES ({})",
//                     DB_TABLE_NAME,
//                     columns_str.join(", "),
//                     placeholders.join(", ")
//                 );

//                 // println!("Query: {}", query);  // Debug print
//                 // println!("Filename: {}", filename);  // Debug print
//                 // println!("Hash: {}", filename_hash);  // Debug print

//                 // Get a connection from the pool
//                 let mut conn = pool.get_conn()?;

//                 // Remove filename_hash from query_params if it exists
//                 // query_params.retain(|(key, _)| key != "filename_hash");

//                 // Ensure we're using Value::from with the complete hash string
//                 // values.push(Value::from(filename_hash.clone()));

//                 // Execute with explicit debug information
//                 match conn.exec_drop(query, values.clone()) {
//                     Ok(_) => println!("Successfully inserted record for {} with hash {}", filename, filename_hash),
//                     Err(e) => eprintln!("Error inserting {} (hash: {}): {}", filename, filename_hash, e),
//                 }
//             }

//             if let Err(_) = fs::remove_file(&dem_path) {
//                 continue;
//             }
//         }
//     Ok(())
// }

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
    let new_dir = env::current_dir().unwrap().join("demos");
    env::set_current_dir(&new_dir).unwrap();

    let path = Path::new("all_demos.json");
    if !path.exists() {
        OpenOptions::new().write(true).create_new(true).open(path)?;
    }

    // Load .env from the project root (parent of demos directory)
    let env_path = env::current_dir()?.parent().unwrap().join(".env");
    match dotenv::from_path(&env_path) {
        Ok(_) => println!(".env file loaded successfully from {:?}", env_path),
        Err(e) => eprintln!("Error loading .env file from {:?}: {}", env_path, e),
    }

    let mut processed_files = HashSet::new();
    batchparse(&mut processed_files)?;
    // batchparse_database_nojson()?;
    Ok(())
}

