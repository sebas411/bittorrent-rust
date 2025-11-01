mod modules;
use std::{env, fs};

use crate::modules::{bencode::decode_bencoded_value, torrent::Torrent};

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
            let torrent = Torrent::new(decoded_value).unwrap();
            torrent.print_info();
        },
        _ => {
            println!("unknown command: {}", args[1])
        },
    }
}
