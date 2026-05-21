use std::io::{BufRead, Cursor, Seek};

use exif::{In, Reader, Tag, Value};

fn read_orientation_from_reader<R: BufRead + Seek>(mut reader: R) -> Option<u16> {
    let exif = Reader::new().read_from_container(&mut reader).ok()?;
    let field = exif.get_field(Tag::Orientation, In::PRIMARY)?;
    match field.value {
        Value::Short(ref v) => v.first().copied(),
        _ => None,
    }
}

/// Read the EXIF Orientation tag from a byte slice, returning a value in 1..=8.
/// Returns None if the bytes have no EXIF or no orientation tag.
pub fn read_orientation_from_bytes(bytes: &[u8]) -> Option<u16> {
    read_orientation_from_reader(Cursor::new(bytes))
}

/// Read the EXIF Orientation tag for a file, returning a value in 1..=8.
/// Returns None if the file has no EXIF or no orientation tag.
#[cfg(feature = "fs")]
pub fn read_orientation(path: &std::path::Path) -> Option<u16> {
    let file = std::fs::File::open(path).ok()?;
    read_orientation_from_reader(std::io::BufReader::new(file))
}
