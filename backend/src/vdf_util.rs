use byteorder::{LittleEndian, ReadBytesExt};
use serde::ser::Error;
use serde_json::{self, Value};
use std::io::{self, Read};

#[derive(Debug)]
enum DataType {
    None = 0,
    String = 1,
    Integer32 = 2,
    Float = 3,
    Pointer = 4,
    WideString = 5,
    Color = 6,
    UnsignedInteger64 = 7,
    End = 8,
    Integer64 = 10,
    EndAlt = 11,
}

pub fn binary_to_json<T: Read>(reader: &mut T) -> serde_json::Result<Value> {
    let mut json_obj = serde_json::Map::new();
    let mut buffer = Vec::new();

    loop {
        match DataType::from_u8(reader.read_u8().unwrap()) {
            Some(DataType::End) | Some(DataType::EndAlt) => return Ok(Value::Object(json_obj)),
            Some(data_type) => {
                let key = read_string(reader, &mut buffer).unwrap();
                let value = match data_type {
                    DataType::None => binary_to_json(reader)?,
                    DataType::String => Value::String(read_string(reader, &mut buffer).unwrap()),
                    DataType::Integer32 | DataType::Pointer | DataType::Color => Value::Number(
                        serde_json::Number::from(reader.read_u32::<LittleEndian>().unwrap()),
                    ),
                    DataType::Float => Value::Number(
                        serde_json::Number::from_f64(reader.read_f64::<LittleEndian>().unwrap())
                            .unwrap(),
                    ),
                    // todo: Assuming WSTRING is treated as a regular string
                    DataType::WideString => {
                        Value::String(read_string(reader, &mut buffer).unwrap())
                    }
                    DataType::Integer64 => Value::Number(serde_json::Number::from(
                        reader.read_i64::<LittleEndian>().unwrap(),
                    )),
                    DataType::UnsignedInteger64 => Value::Number(serde_json::Number::from(
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
            0 => Some(DataType::None),
            1 => Some(DataType::String),
            2 => Some(DataType::Integer32),
            3 => Some(DataType::Float),
            4 => Some(DataType::Pointer),
            5 => Some(DataType::WideString),
            6 => Some(DataType::Color),
            7 => Some(DataType::UnsignedInteger64),
            8 => Some(DataType::End),
            10 => Some(DataType::Integer64),
            11 => Some(DataType::EndAlt),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::vdf_util::binary_to_json;
    use serde_json::json;

    #[test]
    fn test_binary_to_json() {
        // Usage example
        // This is a binary representation of the following JSON:
        // {"name": "John", "age": 30, "city": "New York"}
        let binary_data = vec![
            0x01, 0x6E, 0x61, 0x6D, 0x65, 0x00, 0x4A, 0x6F, 0x68, 0x6E, 0x00, // name: John
            0x02, 0x61, 0x67, 0x65, 0x00, 0x1E, 0x00, 0x00, 0x00, // age: 30
            0x01, 0x63, 0x69, 0x74, 0x79, 0x00, 0x4E, 0x65, 0x77, 0x20, 0x59, 0x6F, 0x72, 0x6B,
            0x00, // city: New York
            0x08, // End
        ];

        let result = binary_to_json(&mut &binary_data[..]).unwrap();
        let expected_json = json!({
            "name": "John",
            "age": 30,
            "city": "New York"
        });

        assert_eq!(result, expected_json);
    }
}
