use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use exif::{In, Reader, Tag, Value};

/// Read the EXIF Orientation tag for a file, returning a value in 1..=8.
/// Returns None if the file has no EXIF or no orientation tag.
pub fn read_orientation(path: &Path) -> Option<u16> {
    let file = File::open(path).ok()?;
    let mut reader = BufReader::new(file);
    let exif = Reader::new().read_from_container(&mut reader).ok()?;
    let field = exif.get_field(Tag::Orientation, In::PRIMARY)?;
    match field.value {
        Value::Short(ref v) => v.first().copied(),
        _ => None,
    }
}
