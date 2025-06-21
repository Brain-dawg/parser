use zmq::Context;
use std::process::Command;
use serde::{Deserialize, Serialize};
use std::env;
use log::{info, error};

#[derive(Serialize, Deserialize)]
struct CompressionJob {
    file: String,
    output: String,
}

fn main() {
    env_logger::init();
    println!("Starting compression worker");
    info!("Starting compression worker");
    
    let push_socket = env::var("PUSH_SOCKET").unwrap_or_else(|_| "6579".to_string());
    let pull_socket = env::var("PULL_SOCKET").unwrap_or_else(|_| "6578".to_string());
    let host = env::var("HOST").unwrap_or_else(|_| "storage.potato.tf".to_string());

    let context = Context::new();

    let receiver = context.socket(zmq::PULL).unwrap();
    receiver.connect(&format!("tcp://{}:{}", host, pull_socket)).unwrap();

    let sender = context.socket(zmq::PUSH).unwrap();
    sender.connect(&format!("tcp://{}:{}", host, push_socket)).unwrap();

    loop {

        let msg = receiver.recv_string(0).unwrap().unwrap();
        let job: CompressionJob = serde_json::from_str(&msg).unwrap();

        println!("Received compression job for {}", job.file);
        info!("Received compression job for {}", job.file);

        let output = Command::new("xz")
            .arg("-9")
            .arg(&job.file)
            .output()
            .expect("Failed to compress file");

        if !output.status.success() {
            let errmsg = String::from_utf8_lossy(&output.stderr);
            println!("Failed to compress file: {}", errmsg);
            error!("Failed to compress file: {}", errmsg);
        }

        println!("Compression job completed for {}", job.file);
        info!("Compression job completed for {}", job.file);
        sender.send(&format!("{}", job.output), 0).unwrap();
    }
}