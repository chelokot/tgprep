use serde::Serialize;
use serde_json::ser::{CompactFormatter, Formatter};
use std::io;

const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

struct JsFormatter;

impl Formatter for JsFormatter {
    fn write_f64<W: ?Sized + io::Write>(&mut self, writer: &mut W, value: f64) -> io::Result<()> {
        if value.fract() == 0.0 && value.abs() <= MAX_SAFE_INTEGER {
            #[expect(clippy::cast_possible_truncation, reason = "integral and within the safe range")]
            let integer = value as i64;
            CompactFormatter.write_i64(writer, integer)
        } else {
            CompactFormatter.write_f64(writer, value)
        }
    }
}

pub fn stringify<T: Serialize>(value: &T) -> serde_json::Result<String> {
    let mut buffer = Vec::new();
    value.serialize(&mut serde_json::Serializer::with_formatter(&mut buffer, JsFormatter))?;
    String::from_utf8(buffer).map_err(|error| serde::ser::Error::custom(error))
}
