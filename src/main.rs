use std::io::{Write, Read};
use std::net::{TcpStream};
use std::thread;
use std::time::Duration;
use std::str;
use reqwest;
use sha1::{Sha1, Digest};

const USERNAME: &str = "ArduWallet";
const MINING_KEY: &str = "";
const USE_LOWER_DIFF: bool = false;
const DISPLAY_NAME: &str = "RustMiner";

fn fetch_pools() -> (String, u16) {
    loop {
        match reqwest::blocking::get("https://server.duinocoin.com/getPool") {
            Ok(response) => {
                if let Ok(json) = response.json::<serde_json::Value>() {
                    return (json["ip"].as_str().unwrap().to_string(), json["port"].as_u64().unwrap() as u16);
                }
            }
            Err(_) => {
                println!("Error retrieving mining node, retrying in 15s");
                thread::sleep(Duration::from_secs(15));
            }
        }
    }
}

fn solve_job(job: Vec<&str>) -> (u64, f64) {
    let hashing_start_time = std::time::Instant::now();
    let mut hasher = Sha1::new();
    hasher.update(job[0].as_bytes());
    let base_hash = hasher.clone();

    for result in 0..(100 * job[2].parse::<u64>().unwrap() + 1) {
        // Calculate hash
        let mut temp_hash = base_hash.clone();
        temp_hash.update(result.to_string().as_bytes());
        let ducos1 = format!("{:x}", temp_hash.finalize_reset());

        // Check if correct
        if job[1] == ducos1 {
            let hashing_stop_time = std::time::Instant::now();
            let time_difference = hashing_stop_time.duration_since(hashing_start_time).as_secs_f64();
            let hashrate = result as f64 / time_difference;
            return (result, hashrate);
        }
    }
    (0, 0.0) // Return Fallback
}

fn main() {
    loop {
        let (node_address, node_port) = match fetch_pools() {
            (addr, port) => (addr, port),
        };

        // Socket
        let mut soc = TcpStream::connect(format!("{}:{}", node_address, node_port)).expect("Failed to connect to server");
        let mut buffer = [0; 100];
        soc.read(&mut buffer).expect("Failed to read from server");
        println!("Server Version: {}", str::from_utf8(&buffer).unwrap());

        // Mine
        loop {
            let difficulty = if USE_LOWER_DIFF { "LOW" } else { "MEDIUM" };
            let job_request = format!("JOB,{},{},{}", USERNAME, difficulty, MINING_KEY);
            soc.write_all(job_request.as_bytes()).expect("Failed to send job request");

            // Receive job
            let mut job_buffer = [0; 1024];
            let bytes_read = soc.read(&mut job_buffer).expect("Failed to read job from server");
            let job = str::from_utf8(&job_buffer[..bytes_read]).unwrap().trim_end().split(',').collect::<Vec<&str>>();

            let (result, hashrate) = solve_job(job.clone());

            // Send result
            let result_message = format!("{},{},{}", result, hashrate, DISPLAY_NAME);
            soc.write_all(result_message.as_bytes()).expect("Failed to send result");

            // Get feedback
            let mut feedback_buffer = [0; 1024];
            let bytes_read = soc.read(&mut feedback_buffer).expect("Failed to read feedback from server");
            let feedback = str::from_utf8(&feedback_buffer[..bytes_read]).unwrap().trim_end();

            // Process feedback
            match feedback {
                "GOOD" => println!("Accepted share {} Hashrate {} kH/s Difficulty {}", result, (hashrate / 1000.0).round() as u64, job[2]),
                "BAD" => println!("Rejected share {} Hashrate {} kH/s Difficulty {}", result, (hashrate / 1000.0).round() as u64, job[2]),
                _ => println!("Malformed share: {} {} Hashrate {} kH/s Difficulty {}", feedback, result, (hashrate / 1000.0).round() as u64, job[2]),
            }
        }
    }
}

