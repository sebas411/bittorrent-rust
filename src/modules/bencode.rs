use crate::modules::value::{Map, Value};

fn is_digit(c: char) -> bool {
    c >= '0' && c <= '9'
}

fn find_index(vector: &[u8], c: char) -> usize {
    vector.iter().position(|u| *u == c as u8).unwrap()
}

pub fn encode_value(value: Value) -> Vec<u8> {
    match value {
        Value::Int(val) => format!("i{}e", val.to_string()).as_bytes().to_vec(),
        Value::String(val) => {
            let mut bencoded = vec![];
            bencoded.extend(format!("{}:", val.len()).as_bytes());
            bencoded.extend(val);
            bencoded
        },
        Value::List(list) => {
            let mut bencoded = vec![];
            bencoded.push('l' as u8);
            for value in list {
                bencoded.extend(encode_value(value));
            }
            bencoded.push('e' as u8);
            bencoded
        },
        Value::Map(map) => {
            let mut bencoded = vec![];
            bencoded.push('d' as u8);
            for (key, val) in map {
                bencoded.extend(encode_value(Value::String(key)));
                bencoded.extend(encode_value(val));
            }
            bencoded.push('e' as u8);
            bencoded
        }
    }
}

pub fn decode_bencoded_value(encoded_value: &[u8]) -> (Value, Vec<u8>) {
    if is_digit(encoded_value[0] as char) {
        // Example: "5:hello" -> "hello"
        let colon_index = find_index(encoded_value, ':');
        let number_string = String::from_utf8(encoded_value[..colon_index].to_vec()).unwrap();
        let number = number_string.parse::<usize>().unwrap();
        let string = &encoded_value[colon_index + 1..colon_index + 1 + number];
        return (Value::String(string.to_vec()), encoded_value[colon_index + 1 + number..].to_vec());
    } else if encoded_value[0] as char == 'i' {
        // Example: "i-52e" -> -52
        let i_index = find_index(encoded_value, 'i');
        let e_index = find_index(encoded_value, 'e');
        let number_string = String::from_utf8(encoded_value[i_index + 1..e_index].to_vec()).unwrap();
        let number = number_string.parse::<i64>().unwrap();
        return (Value::Int(number), encoded_value[e_index+1..].into());
    } else if encoded_value[0] as char == 'l' {
        // Example: "l5:helloi52ee" -> ["hello",52]
        let mut current_string = encoded_value[1..].to_vec();
        let mut value_list = vec![];
        while current_string[0] as char != 'e' {
            let (value, rest) = decode_bencoded_value(&current_string);
            value_list.push(value);
            current_string = rest;
        }
        return (Value::List(value_list), current_string[1..].into())
    } else if encoded_value[0] as char == 'd' {
        // Example: d3:foo3:bar5:helloi52ee
        let mut map = Map::new();
        let mut current_string = encoded_value[1..].to_vec();
        while current_string[0] as char != 'e' {
            let (key, rest) = decode_bencoded_value(&current_string);
            current_string = rest;
            if let Value::String(key) = key {
                let (val, rest) = decode_bencoded_value(&current_string);
                map.insert(key, val);
                current_string = rest;
            } else {
                panic!("Dictionary keys should be Strings.");
            }
        }
        return (Value::Map(map), current_string[1..].into());
    } else {
        panic!("Unhandled encoded value: {}", String::from_utf8(encoded_value.to_vec()).unwrap())
    }
}