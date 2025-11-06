mod modules;
use std::{env, fs::{self, File}, io::{Read, Write}, net::TcpStream};
use percent_encoding::{NON_ALPHANUMERIC, percent_encode};
use rand::{distr::{Alphanumeric, SampleString}};
use rand::rng;
use sha1::{Digest, Sha1};

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

fn generate_random_string(length: usize) -> String {
    Alphanumeric::default().sample_string(&mut rng(), length)
}

fn get_peers(torrent: &Torrent, peer_id: &str) -> Vec<(String, u16)> {
    let req_url = torrent.get_url();
    let info_hash = torrent.info.get_info_hash_bytes();
    let info_hash_encoded = percent_encode(&info_hash, NON_ALPHANUMERIC).to_string();
    let port = 6881;
    let file_size = torrent.info.get_length();
    let query_params = format!("?info_hash={}&peer_id={}&port={}&uploaded=0&downloaded=0&left={}&compact=1", info_hash_encoded, peer_id, port, file_size);
    let url = req_url + &query_params;
    let response = reqwest::blocking::get(url).unwrap().bytes().unwrap();
    let resp_dict = decode_bencoded_value(&response).0.get_map().unwrap();
    let peers = resp_dict.get("peers").unwrap().get_string().unwrap();
    let peers = bytes_to_peer_list(&peers);
    peers
}

fn get_handshake(torrent: &Torrent, peer_id: &str) -> Vec<u8> {
    let mut handshake = vec![];
    handshake.push(19u8);                                        // length of protocol string
    handshake.extend("BitTorrent protocol".as_bytes());    // protocol
    handshake.extend(&[0u8; 8]);                           // reserved bytes
    handshake.extend(torrent.info.get_info_hash_bytes());  // info hash
    handshake.extend(peer_id.as_bytes());                  // peer id
    handshake
}

fn download_piece(torrent: &Torrent, self_id: &str, peer: &str, piece_index: usize) -> Vec<u8> {
    let handshake = get_handshake(torrent, self_id);
    let piece_hash = torrent.info.get_piece(piece_index);
    let mut piece_size = torrent.info.get_piece_size();
    let file_size = torrent.info.get_file_size();

    if file_size < (piece_index + 1) * piece_size {
        piece_size = file_size % piece_size;
    }


    // handshake
    let mut stream = TcpStream::connect(peer).expect("Failed to connect");
    stream.write_all(&handshake).expect("Failed to write to stream");

    let mut buffer = [0; 1024 * 16 + 4 + 1 + 8];
    stream.read_exact(&mut buffer[0..1]).expect("Failed to read from stream");
    let protocol_length = buffer[0] as usize;
    stream.read_exact(&mut buffer[1..1+protocol_length+8+20+20]).unwrap(); // dont care about this, but we read it to free the buffer

    // wait for bitfield
    stream.read(&mut buffer).expect("Failed to read from stream");
    let message_type = buffer[4];
    if message_type != 5 {
        panic!("Didn't get bitfield message {}", message_type);
    }

    // send interested
    let mut message_content = 1u32.to_be_bytes().to_vec();
    message_content.push(2u8);
    stream.write_all(&message_content).expect("Failed to write to stream");

    // wait for unchoke
    stream.read(&mut buffer).expect("Failed to read from stream");
    let message_type = buffer[4];
    if message_type != 1 {
        panic!("Didn't get unchoke message");
    }

    let max_block_size = 1024 * 16;

    // request blocks
    let mut i = 0;
    while i*max_block_size < piece_size {
        let offset = (i*max_block_size) as u32;
        let mut size =  max_block_size as u32;
        if size + offset > piece_size as u32 {
            size = (piece_size % max_block_size) as u32;
        }
        let mut message = 13u32.to_be_bytes().to_vec();
        message.push(6);
        message.extend((piece_index as u32).to_be_bytes());
        message.extend(offset.to_be_bytes());
        message.extend(size.to_be_bytes());
        stream.write_all(&message).expect("Failed to write to stream");

        i += 1;
    }

    let total_blocks = i;

    // get blocks
    let mut blocks = vec![];
    for _ in 0..total_blocks {
        stream.read_exact(&mut buffer[0..5]).expect("Failed to read");
        let message_type = buffer[4];
        if message_type != 7 {
            panic!("Didn't get piece_block, got {}", message_type, )
        }
        let data_size = u32::from_be_bytes(buffer[0..4].try_into().unwrap()) - 1 - 8;
        stream.read_exact(&mut buffer[5..13+(data_size as usize)]).expect("Failed to read");
        let begin = u32::from_be_bytes(buffer[9..13].try_into().unwrap());
        let block_data =  buffer[13..13+(data_size as usize)].to_vec();
        blocks.push((begin, block_data));
    }

    blocks.sort_by(|a, b| a.0.cmp(&b.0));
    let mut piece = vec![];
    for block in blocks {
        piece.extend(block.1);
    }

    // verify piece hash
    let mut hasher = Sha1::new();
    hasher.update(&piece);
    let sha_hash = hasher.finalize();
    let sha_hash: [u8; 20] = sha_hash.try_into().unwrap();
    if piece_hash != sha_hash {
        panic!("Piece hash doesn't match.")
    }
    piece
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
            let peers = get_peers(&torrent, &peer_id);
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
            let handshake = get_handshake(&torrent, &self_id);

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
            let peers = get_peers(&torrent, &my_id);
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
            let peers = get_peers(&torrent, &my_id);
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
        _ => {
            println!("unknown command: {}", args[1])
        },
    }
}
