mod modules;
use std::{env, fs::{self, File}, io::{Read, Write}, net::TcpStream};
use rand::{distr::{Alphanumeric, SampleString}};
use rand::rng;

use crate::modules::{bencode::{decode_bencoded_value, encode_value}, helpers::{download_piece, get_handshake, get_peers}, torrent::{Magnet, Torrent}, value::{Map, Value}};

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

fn generate_random_string(length: usize) -> String {
    Alphanumeric::default().sample_string(&mut rng(), length)
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
            
            let peer_id = generate_random_string(20);
            let peers = get_peers(&torrent.get_url(), &torrent.info.get_info_hash_bytes(), &peer_id, torrent.info.get_file_size());
            for peer in peers {
                println!("{}:{}", peer.0, peer.1)
            }

        },
        "handshake" => {
            let filename = &args[2];
            let contents = fs::read(filename).unwrap();
            let (decoded_value, _) = decode_bencoded_value(&contents);
            let torrent = Torrent::new(decoded_value).unwrap();
            let peer = &args[3];

            let self_id = generate_random_string(20);
            let handshake = get_handshake(&torrent.info.get_info_hash_bytes(), &self_id, false);

            let mut stream = TcpStream::connect(peer).expect("Failed to connect");
            stream.write_all(&handshake).expect("Failed to write to stream");

            let mut buffer = [0; 1024];
            stream.read(&mut buffer).expect("Failed to read from stream");
            let protocol_length = buffer[0] as usize;
            let start = 1 + protocol_length + 8 + 20;
            let peer_id = buffer[start..start+20].to_vec();
            let peer_id = hex::encode(peer_id);
            println!("Peer ID: {}", peer_id);
        },
        "download_piece" => {
            //  -o /tmp/test-piece sample.torrent <piece_index>
            let mut values_set = (false, false, false);
            let mut set_storage_location = false;
            let mut storage_location = String::new();
            let mut filename = String::new();
            let mut piece_index = 0;
            for i in 2..args.len() {
                let arg = &args[i];
                if set_storage_location {
                    set_storage_location = false;
                    storage_location = arg.into();
                    values_set.0 = true;
                    continue;
                }
                if arg == "-o" {
                    set_storage_location = true;
                    continue;
                }
                if !values_set.1 {
                    filename = arg.into();
                    values_set.1 = true;
                } else if !values_set.2 {
                    piece_index = usize::from_str_radix(arg, 10).unwrap();
                    values_set.2 = true;
                } else {
                    panic!("Unexpected parameter for download_piece");
                }
            }
            if values_set != (true, true, true) {
                panic!("Missing parameters for download_piece")
            }
            let content = fs::read(filename).unwrap();
            let (decoded_value, _) = decode_bencoded_value(&content);
            let torrent = Torrent::new(decoded_value).unwrap();
            let my_id = generate_random_string(20);
            let peers = get_peers(&torrent.get_url(), &torrent.info.get_info_hash_bytes(), &my_id, torrent.info.get_file_size());
            let peer = format!("{}:{}", peers[0].0, peers[0].1);
            let piece = download_piece(&torrent, &my_id, &peer, piece_index);
            let mut file = File::create(storage_location).unwrap();
            file.write_all(&piece).unwrap();
            println!("Piece downloaded.");
        },
        "download" => {
            // download -o /tmp/test.txt sample.torrent
            let mut set_storage_location = false;
            let mut storage_location = String::new();
            let mut filename = String::new();
            let mut values_set = (false, false);
            for i in 2..args.len() {
                let arg = &args[i];
                if set_storage_location {
                    set_storage_location = false;
                    storage_location = arg.into();
                    values_set.0 = true;
                    continue;
                }
                if arg == "-o" {
                    set_storage_location = true;
                    continue;
                }
                if !values_set.1 {
                    filename = arg.into();
                    values_set.1 = true;
                } else {
                    panic!("Unexpected parameter for download_piece");
                }
            }
            if values_set != (true, true) {
                panic!("Missing parameters for download")
            }
            let content = fs::read(filename).unwrap();
            let (decoded_value, _) = decode_bencoded_value(&content);
            let torrent = Torrent::new(decoded_value).unwrap();
            let my_id = generate_random_string(20);
            let peers = get_peers(&torrent.get_url(), &torrent.info.get_info_hash_bytes(), &my_id, torrent.info.get_file_size());
            let piece_num = torrent.info.total_pieces();
            let mut file_contents = vec![];
            for i in 0..piece_num {
                let peer = format!("{}:{}", peers[i%peers.len()].0, peers[i%peers.len()].1);
                file_contents.extend(download_piece(&torrent, &my_id, &peer, i));
            }
            let mut file = File::create(storage_location).unwrap();
            file.write_all(&file_contents).unwrap();
            println!("File downloaded.")
        },
        "magnet_parse" => {
            let magnet_link = &args[2];
            let magnet = Magnet::new(magnet_link).unwrap();
            magnet.print_info();
        },
        "magnet_handshake" => {
            let magnet_link = &args[2];
            let magnet = Magnet::new(magnet_link).unwrap();
            let my_id = generate_random_string(20);
            let info_hash = magnet.get_info_hash_bytes();
            let handshake = get_handshake(&info_hash, &my_id, true);
            let peers = get_peers(&magnet.get_url().unwrap(), &info_hash, &my_id, 999);
            let peer = format!("{}:{}", peers[0].0, peers[0].1);

            let mut stream = TcpStream::connect(peer).expect("Failed to connect");
            stream.write_all(&handshake).expect("Failed to write to stream");

            let mut buffer = [0; 1024];
            stream.read_exact(&mut buffer[0..1]).expect("Failed to read from stream");
            let protocol_length = buffer[0] as usize;
            stream.read_exact(&mut buffer[1..1+protocol_length+8+20+20]).unwrap();
            let has_extension_support = buffer[1+protocol_length+5] & 16u8 > 0;
            let start = 1 + protocol_length + 8 + 20;
            let peer_id = buffer[start..start+20].to_vec();
            let peer_id = hex::encode(peer_id);

            // wait for bitfield
            stream.read_exact(&mut buffer[..5]).expect("Failed to read from stream");
            let message_type = buffer[4];
            if message_type != 5 {
                panic!("Didn't get bitfield message {}", message_type);
            }
            
            //extension handshake
            if has_extension_support {
                let mut inner_dict = Map::new();
                inner_dict.insert("ut_metadata".as_bytes().to_vec(), Value::Int(1));
                let mut outer_dict = Map::new();
                outer_dict.insert("m".as_bytes().to_vec(), Value::Map(inner_dict));
                let bencoded_value = encode_value(Value::Map(outer_dict));
                let mut extension_handshake = vec![];
                extension_handshake.extend((bencoded_value.len() as u32 + 2).to_be_bytes());
                extension_handshake.push(20);
                extension_handshake.push(0);
                extension_handshake.extend(bencoded_value);
                stream.write_all(&extension_handshake).expect("Couldn't write to stream");

                stream.read(&mut buffer).expect("Couldn't read from stream");
            }

            println!("Peer ID: {}", peer_id);
        },
        _ => {
            println!("unknown command: {}", args[1])
        },
    }
}
