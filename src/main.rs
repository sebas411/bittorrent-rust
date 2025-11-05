mod modules;
use std::{env, fs};
use percent_encoding::{NON_ALPHANUMERIC, percent_encode};

use crate::modules::{bencode::decode_bencoded_value, torrent::Torrent};

fn bytes_to_peer_list(bytes: &[u8]) -> Vec<(String, u16)> {
    let mut i = 0;
    let mut peers = vec![];
    while bytes.len() >= i+6 {
        let ip = format!("{}.{}.{}.{}", bytes[i], bytes[i+1], bytes[i+2], bytes[i+3]);
        let port = u16::from_be_bytes([bytes[i+4], bytes[i+5]]);
        i += 6;
        peers.push((ip, port));
    }
    peers
}

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
        "peers" => {
            let filename = &args[2];
            let contents = fs::read(filename).unwrap();
            let (decoded_value, _) = decode_bencoded_value(&contents);
            let torrent = Torrent::new(decoded_value).unwrap();

            let req_url = torrent.get_url();
            let info_hash = torrent.info.get_info_hash_bytes();
            let info_hash_encoded = percent_encode(&info_hash, NON_ALPHANUMERIC).to_string();
            let peer_id = String::from("sebas411_bittor_peer");
            let port = 6881;
            let file_size = torrent.info.get_length();
            let query_params = format!("?info_hash={}&peer_id={}&port={}&uploaded=0&downloaded=0&left={}&compact=1", info_hash_encoded, peer_id, port, file_size);
            let url = req_url + &query_params;
            let response = reqwest::blocking::get(url).unwrap().bytes().unwrap();
            let resp_dict = decode_bencoded_value(&response).0.get_map().unwrap();
            let peers = resp_dict.get("peers").unwrap().get_string().unwrap();
            let peers = bytes_to_peer_list(&peers);
            for peer in peers {
                println!("{}:{}", peer.0, peer.1)
            }

        },
        _ => {
            println!("unknown command: {}", args[1])
        },
    }
}
