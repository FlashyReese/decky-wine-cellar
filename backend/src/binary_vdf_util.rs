// Port of <https://github.com/Grub4K/VDFparse/blob/main/VDFparse/KVTransformer.cs> under MIT

use byteorder::{LittleEndian, ReadBytesExt};
use serde::ser::Error;
use serde_json::{self, Value};
use std::io::{self, Read};

#[derive(Debug)]
enum DataType {
    START = 0,
    STRING,
    INT,
    FLOAT,
    PTR,
    WSTRING,
    COLOR,
    UINT64,
    END,
    INT64 = 10,
    ENDB = 11,
}

fn binary_to_json<T: Read>(reader: &mut T) -> serde_json::Result<Value> {
    let mut json_obj = serde_json::Map::new();
    let mut buffer = Vec::new();

    loop {
        match DataType::from_u8(reader.read_u8().unwrap()) {
            Some(DataType::END) | Some(DataType::ENDB) => return Ok(Value::Object(json_obj)),
            Some(data_type) => {
                let key = read_string(reader, &mut buffer).unwrap();
                let value = match data_type {
                    DataType::START => binary_to_json(reader)?,
                    DataType::STRING => {
                        Value::String(read_string(reader, &mut buffer).unwrap().into())
                    }
                    DataType::INT => Value::Number(serde_json::Number::from(
                        reader.read_u32::<LittleEndian>().unwrap(),
                    )),
                    DataType::FLOAT => Value::Number(
                        serde_json::Number::from_f64(reader.read_f64::<LittleEndian>().unwrap())
                            .unwrap(),
                    ),
                    DataType::PTR => Value::Number(serde_json::Number::from(
                        reader.read_u32::<LittleEndian>().unwrap(),
                    )),
                    DataType::WSTRING => {
                        Value::String(read_string(reader, &mut buffer).unwrap().into())
                    } // Assuming WSTRING is treated as a regular string
                    DataType::COLOR => {
                        let color_value = reader.read_u32::<LittleEndian>().unwrap();
                        Value::String(format!("#{:08X}", color_value))
                    }
                    DataType::INT64 => Value::Number(serde_json::Number::from(
                        reader.read_i64::<LittleEndian>().unwrap(),
                    )),
                    DataType::UINT64 => Value::Number(serde_json::Number::from(
                        reader.read_u64::<LittleEndian>().unwrap(),
                    )),
                    _ => {
                        return Err(serde_json::Error::custom(format!(
                            "Unexpected type for value ({:?})",
                            data_type
                        )));
                    }
                };
                json_obj.insert(key, value);
            }
            None => {
                return Err(serde_json::Error::custom("Invalid data type"));
            }
        }
    }
}

fn read_string<T: Read>(reader: &mut T, buffer: &mut Vec<u8>) -> io::Result<String> {
    buffer.clear();
    let mut byte = [0u8];
    loop {
        reader.read_exact(&mut byte)?;
        if byte[0] == 0 {
            break;
        }
        buffer.push(byte[0]);
    }
    Ok(String::from_utf8_lossy(buffer).into_owned())
}

impl DataType {
    fn from_u8(value: u8) -> Option<DataType> {
        match value {
            0 => Some(DataType::START),
            1 => Some(DataType::STRING),
            2 => Some(DataType::INT),
            3 => Some(DataType::FLOAT),
            4 => Some(DataType::PTR),
            5 => Some(DataType::WSTRING),
            6 => Some(DataType::COLOR),
            7 => Some(DataType::UINT64),
            8 => Some(DataType::END),
            10 => Some(DataType::INT64),
            11 => Some(DataType::ENDB),
            _ => None,
        }
    }
}

mod tests {
    use crate::binary_vdf_util::binary_to_json;
    use std::fs::File;
    use std::io::{Cursor, Read};

    #[test]
    fn binary_to_json_test() {
        let file_path = "/home/flashyreese/.steam/steam/userdata/156426923/config/shortcuts.vdf";

        let mut binary_data = Vec::new();
        match File::open(file_path) {
            Ok(mut file) => {
                if let Err(err) = file.read_to_end(&mut binary_data) {
                    eprintln!("Failed to read file: {:?}", err);
                    return;
                }
            }
            Err(err) => {
                eprintln!("Failed to open file: {:?}", err);
                return;
            }
        }

        let mut cursor = Cursor::new(binary_data);
        match binary_to_json(&mut cursor) {
            Ok(json_value) => {
                let pretty_json = serde_json::to_string_pretty(&json_value).unwrap();
                println!("{}", pretty_json);
            }
            Err(err) => {
                eprintln!("Failed to convert binary to JSON: {:?}", err);
            }
        }
    }
}
