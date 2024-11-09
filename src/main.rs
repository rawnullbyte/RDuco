use std::io::{Write, Read};
use std::net::{TcpStream};
use std::thread;
use std::time::Duration;
use std::str;
use reqwest;
use sha1::{Sha1, Digest};
use std::process::Command;
use crossterm::style::{Color, SetForegroundColor, ResetColor};
use std::io;

const ASK_DATA: bool = false;
const DEFAULT_USERNAME: &str = "ArduWallet";
const DEFAULT_MINING_KEY: &str = "";
const DEFAULT_USE_LOWER_DIFF: bool = false;
const DEFAULT_SOFTWARE: &str = "Official PC Miner 3.5"; // ['Official PC Miner 3.5', 'Official ESP8266 Miner 3.5', 'Official ESP32 Miner 3.5', 'Duino-Coin AVR Miner 4.2'
const DEFAULT_IDENTIFIER: &str = "Rust1"; // RIG1 - RPi
const DEFAULT_CHIP_ID: &str = "1";

fn fetch_pools() -> (String, u16) {
    loop {
        match reqwest::blocking::get("https://server.duinocoin.com/getPool") {
            Ok(response) => {
                if let Ok(json) = response.json::<serde_json::Value>() {
                    return (json["ip"].as_str().unwrap().to_string(), json["port"].as_u64().unwrap() as u16);
                }
            }
            Err(_) => {
                println!("{}Error retrieving mining node, retrying in 15s{}", SetForegroundColor(Color::Red), ResetColor);
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

fn get_cpu_temp() -> String {
    if cfg!(target_os = "windows") {
        let output = Command::new("powershell")
            .arg("-NoProfile")
            .arg("-Command")
            .arg("Get-WmiObject MSAcpi_ThermalZoneTemperature -Namespace \"root/wmi\" | Select-Object -ExpandProperty CurrentTemperature")
            .output()
            .expect("Failed to execute command");

        if let Ok(temp_str) = String::from_utf8(output.stdout) {
            if let Ok(temp_kelvin) = temp_str.trim().parse::<f64>() {
                let celsius_temp = (temp_kelvin / 10.0) - 273.15;
                return format!("{:.1}", celsius_temp);
            }
        }
    }
    "0".to_string()
}

fn main() {
    let (username, mining_key, identifier, chip_id, software, use_lower_diff) = if ASK_DATA {
        let mut input = String::new();
        println!("Enter Username[ArduWallet]:");
        io::stdin().read_line(&mut input).expect("Failed to read line");
        let mut username = input.trim().to_string();
        if username == "" {
            username = DEFAULT_USERNAME.to_string();
        }

        input.clear();
        println!("Enter Mining Key[]:");
        io::stdin().read_line(&mut input).expect("Failed to read line");
        let mining_key = input.trim().to_string();

        input.clear();
        println!("Use lower difficulty[y/n]:");
        io::stdin().read_line(&mut input).expect("Failed to read line");
        let mut use_lower_diff = input.trim().to_string();
        if use_lower_diff == "" {
            use_lower_diff = DEFAULT_USE_LOWER_DIFF.to_string();
        }

        input.clear();
        println!("Enter Software[Official PC Miner 3.5]:");
        io::stdin().read_line(&mut input).expect("Failed to read line");
        let mut software = input.trim().to_string();
        if software == "" {
            software = DEFAULT_SOFTWARE.to_string();
        }

        input.clear();
        println!("Enter Identifier[RustMiner]:");
        io::stdin().read_line(&mut input).expect("Failed to read line");
        let mut identifier = input.trim().to_string();
        if identifier == "" {
            identifier = DEFAULT_IDENTIFIER.to_string();
        }

        input.clear();
        println!("Enter Chip ID[1]:");
        io::stdin().read_line(&mut input).expect("Failed to read line");
        let mut chip_id = input.trim().to_string();
        if chip_id == "" {
            chip_id = DEFAULT_CHIP_ID.to_string();
        }

        (username, mining_key, identifier, chip_id, software, use_lower_diff)
    } else {
        (DEFAULT_USERNAME.to_string(), DEFAULT_MINING_KEY.to_string(), DEFAULT_IDENTIFIER.to_string(), DEFAULT_CHIP_ID.to_string(), DEFAULT_SOFTWARE.to_string(), DEFAULT_USE_LOWER_DIFF.to_string())
    };

    if get_cpu_temp() == "0" {
        println!("{}Warning: {}Failed to retrieve CPU temperature, try running the script as administrator...", SetForegroundColor(Color::Yellow), ResetColor);
    }
    loop {
        let (node_address, node_port) = match fetch_pools() {
            (addr, port) => (addr, port),
        };

        // Socket
        let mut socket = TcpStream::connect(format!("{}:{}", node_address, node_port)).expect("Failed to connect to server");
        let mut buffer = [0; 100];
        socket.read(&mut buffer).expect("Failed to read from server");
        println!("{}Server Version: {}{}", SetForegroundColor(Color::Yellow), str::from_utf8(&buffer).unwrap().replace("\n", ""), ResetColor);
        println!("{}Logged in as: {}{}", SetForegroundColor(Color::Yellow), if identifier.is_empty() { &software } else { &identifier }, ResetColor);
        println!("{}Account: {}{}", SetForegroundColor(Color::Yellow), username, ResetColor);
        println!("{}Mining key: {}{}", SetForegroundColor(Color::Yellow), mining_key, ResetColor);

        // Mine
        loop {
            let difficulty = if use_lower_diff == "y" { "LOW" } else { "MEDIUM" };
            let job_request = format!("JOB,{},{},{},{}@{}\n", username, difficulty, mining_key, get_cpu_temp(), "0");
            socket.write_all(job_request.as_bytes()).expect("Failed to send job request");

            // Receive job
            let mut job_buffer = [0; 1024];
            let bytes_read = socket.read(&mut job_buffer).expect("Failed to read job from server");
            let job = str::from_utf8(&job_buffer[..bytes_read]).unwrap().trim_end().split(',').collect::<Vec<&str>>();

            let (result, hashrate) = solve_job(job.clone());

            // Send result
            let result_message = format!("{},{},{},{},DUCOID{}", result, hashrate, software, identifier, chip_id);
            socket.write_all(result_message.as_bytes()).expect("Failed to send result");

            // Get feedback
            let mut feedback_buffer = [0; 1024];
            let bytes_read = socket.read(&mut feedback_buffer).expect("Failed to read feedback from server");
            let feedback = str::from_utf8(&feedback_buffer[..bytes_read]).unwrap().trim_end();

            // Process feedback
            let target_width: i32 = 30;  // Adjust this value to set the desired alignment for "|"
            match feedback {
                "GOOD" => {
                    let hashrate_display = if hashrate >= 1_000_000.0 {
                        format!("{:.2} mH/s", hashrate / 1_000_000.0)
                    } else {
                        format!("{} kH/s", (hashrate / 1000.0).round() as u64)
                    };

                    let result_str = result.to_string();
                    let spaces_after_result = target_width.saturating_sub((result_str.len() + "Accepted share ".len()).try_into().unwrap());

                    println!(
                        "{}Accepted share {}{}{}| Hashrate {} | Difficulty {} | Motherboard temp {}°C{}", 
                        SetForegroundColor(Color::Green),
                        SetForegroundColor(Color::Cyan),
                        result_str,
                        " ".repeat(spaces_after_result.try_into().unwrap()),
                        hashrate_display, 
                        job[2], 
                        get_cpu_temp(), 
                        ResetColor
                    );
                }
                "BAD" => {
                    let hashrate_display = if hashrate >= 1_000_000.0 {
                        format!("{:.2} mH/s", hashrate / 1_000_000.0)
                    } else {
                        format!("{} kH/s", (hashrate / 1000.0).round() as u64)
                    };

                    let result_str = result.to_string();
                    let spaces_after_result = target_width.saturating_sub((result_str.len() + "Rejected share ".len()).try_into().unwrap());

                    println!(
                        "{}Rejected share {}{}{}| Hashrate {} | Difficulty {} | Motherboard temp {}°C{}", 
                        SetForegroundColor(Color::Red),
                        SetForegroundColor(Color::Cyan),
                        result_str,
                        " ".repeat(spaces_after_result.try_into().unwrap()),
                        hashrate_display, 
                        job[2], 
                        get_cpu_temp(), 
                        ResetColor
                    );
                }
                _ => {
                    let hashrate_display = if hashrate >= 1_000_000.0 {
                        format!("{:.2} mH/s", hashrate / 1_000_000.0)
                    } else {
                        format!("{} kH/s", (hashrate / 1000.0).round() as u64)
                    };

                    let result_str = result.to_string();
                    let spaces_after_result = target_width.saturating_sub((result_str.len() + "Malformed share: ".len() + feedback.len()).try_into().unwrap());

                    println!(
                        "{}Malformed share: {}{}{}| {} | Hashrate {} | Difficulty {} | Motherboard temp {}°C{}", 
                        SetForegroundColor(Color::Red),
                        SetForegroundColor(Color::Cyan),
                        feedback,
                        result_str,
                        " ".repeat(spaces_after_result.try_into().unwrap()),
                        hashrate_display, 
                        job[2], 
                        get_cpu_temp(), 
                        ResetColor
                    );
                }
            }


        }
    }
}
