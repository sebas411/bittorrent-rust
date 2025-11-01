mod modules;
use std::{env, fs};
use sha1::{Digest, Sha1};

use crate::modules::{bencode::{decode_bencoded_value, encode_value}, value::Value};

fn main() {
    let args: Vec<String> = env::args().collect();
    let command = &args[1];

    match command.as_str() {
        "decode" => {
            let encoded_value = &args[2];
            let (decoded_value, _) = decode_bencoded_value(encoded_value.as_bytes());
            println!("{}", decoded_value);
        },
        "info" => {
            let filename = &args[2];
            let contents = fs::read(filename).unwrap();
            let (decoded_value, _) = decode_bencoded_value(&contents);
            if let Some(torrent_map) = decoded_value.get_map() {
                let tracker_url = torrent_map.get("announce").unwrap();
                let info_map = torrent_map.get("info").unwrap().get_map().unwrap();
                let length = info_map.get("length").unwrap();
                
                let bencoded_info_map = encode_value(Value::Map(info_map));
                let mut hasher = Sha1::new();
                hasher.update(bencoded_info_map);
                let sha1_hash = hasher.finalize();
                let sha1_hash_hex = format!("{:x}", sha1_hash);

                println!("Tracker URL: {}", tracker_url.to_string());
                println!("Length: {}", length.to_string());
                println!("Info Hash: {}", sha1_hash_hex);
            } else {
                panic!("Bad .torrent file")
            }
        },
        _ => {
            println!("unknown command: {}", args[1])
        },
    }
}
