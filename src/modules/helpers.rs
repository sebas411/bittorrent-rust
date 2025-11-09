use std::{io::{Read, Write}, net::TcpStream};
use percent_encoding::{NON_ALPHANUMERIC, percent_encode};
use sha1::{Digest, Sha1};

use crate::{bytes_to_peer_list, modules::{bencode::decode_bencoded_value, torrent::Torrent}};

pub fn get_peers(announce: &str, info_hash: &[u8], peer_id: &str, file_size: usize) -> Vec<(String, u16)> {
    let req_url = String::from(announce);
    let info_hash_encoded = percent_encode(&info_hash, NON_ALPHANUMERIC).to_string();
    let port = 6881;
    let query_params = format!("?info_hash={}&peer_id={}&port={}&uploaded=0&downloaded=0&left={}&compact=1", info_hash_encoded, peer_id, port, file_size);
    let url = req_url + &query_params;
    let result = reqwest::blocking::get(url);
    if let Err(err) = &result {
        println!("Error: {}", err.status().unwrap());
    }
    let response = result.unwrap().bytes().unwrap();
    let resp_dict = decode_bencoded_value(&response).0.get_map().unwrap();
    let peers = resp_dict.get("peers").unwrap().get_string().unwrap();
    let peers = bytes_to_peer_list(&peers);
    peers
}

pub fn get_handshake(info_hash: &[u8], peer_id: &str, metadata_support: bool) -> Vec<u8> {
    let mut handshake = vec![];
    let mut reserved_bytes = [0u8; 8];
    if metadata_support {
        reserved_bytes[5] = 16;
    }
    handshake.push(19u8);                                        // length of protocol string
    handshake.extend("BitTorrent protocol".as_bytes());    // protocol
    handshake.extend(&reserved_bytes);                           // reserved bytes
    handshake.extend(info_hash);  // info hash
    handshake.extend(peer_id.as_bytes());                  // peer id
    handshake
}

pub fn download_piece(torrent: &Torrent, self_id: &str, peer: &str, piece_index: usize) -> Vec<u8> {
    let handshake = get_handshake(&torrent.info.get_info_hash_bytes(), self_id, false);
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
