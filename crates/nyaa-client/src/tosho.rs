//! Streaming reader for AnimeTosho's MariaDB `mysqldump` (extended
//! `INSERT INTO \`t\` (\`cols\`) VALUES (...),(...);` statements).
//!
//! The dump is 11.7 GB unpacked, so nothing is buffered beyond one row: the
//! reader pulls bytes from any `BufRead` (in practice `7z e -so` output),
//! yields statement headers and rows, and can skip a statement it does not
//! need without parsing its values.

use std::io::{self, BufRead};

/// One SQL value. Strings and hex blobs (`0x...`) are both `Bytes`.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Int(i64),
    Float(f64),
    Bytes(Vec<u8>),
}

impl Value {
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Value::Bytes(b) => Some(b),
            _ => None,
        }
    }

    /// The value as text (lossy UTF-8), for string columns.
    pub fn text(&self) -> String {
        match self {
            Value::Bytes(b) => String::from_utf8_lossy(b).into_owned(),
            Value::Int(n) => n.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Null => String::new(),
        }
    }
}

/// The table and column list of an `INSERT` statement.
#[derive(Debug, Clone)]
pub struct Header {
    pub table: String,
    pub columns: Vec<String>,
}

impl Header {
    pub fn index(&self, column: &str) -> Option<usize> {
        self.columns.iter().position(|c| c == column)
    }
}

pub struct InsertReader<R: BufRead> {
    r: R,
}

fn bad(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.to_string())
}

impl<R: BufRead> InsertReader<R> {
    pub fn new(r: R) -> Self {
        Self { r }
    }

    fn peek(&mut self) -> io::Result<Option<u8>> {
        Ok(self.r.fill_buf()?.first().copied())
    }

    fn bump(&mut self) -> io::Result<Option<u8>> {
        let b = self.peek()?;
        if b.is_some() {
            self.r.consume(1);
        }
        Ok(b)
    }

    fn expect(&mut self, want: u8) -> io::Result<()> {
        match self.bump()? {
            Some(b) if b == want => Ok(()),
            other => Err(bad(&format!("expected {:?}, found {:?}", want as char, other.map(|b| b as char)))),
        }
    }

    fn skip_line(&mut self) -> io::Result<()> {
        loop {
            let buf = self.r.fill_buf()?;
            if buf.is_empty() {
                return Ok(());
            }
            match buf.iter().position(|&b| b == b'\n') {
                Some(i) => {
                    self.r.consume(i + 1);
                    return Ok(());
                }
                None => {
                    let n = buf.len();
                    self.r.consume(n);
                }
            }
        }
    }

    /// Reads up to (and consumes) `end`, returning what came before it.
    fn read_until_byte(&mut self, end: u8) -> io::Result<Vec<u8>> {
        let mut out = Vec::new();
        loop {
            match self.bump()? {
                None => return Err(bad("unexpected end of dump in a statement header")),
                Some(b) if b == end => return Ok(out),
                Some(b) => out.push(b),
            }
        }
    }

    /// Advances to the next `INSERT INTO` statement and returns its header,
    /// leaving the cursor on the first row. `None` at the end of the input.
    pub fn next_insert(&mut self) -> io::Result<Option<Header>> {
        const PREFIX: &[u8] = b"INSERT INTO ";
        loop {
            // Match the prefix byte by byte so a non-INSERT line is skipped
            // without reading it whole.
            let mut matched = 0;
            while matched < PREFIX.len() {
                match self.peek()? {
                    None => return Ok(None),
                    Some(b) if b == PREFIX[matched] => {
                        self.r.consume(1);
                        matched += 1;
                    }
                    Some(_) => break,
                }
            }
            if matched < PREFIX.len() {
                self.skip_line()?;
                continue;
            }
            self.expect(b'`')?;
            let table = String::from_utf8_lossy(&self.read_until_byte(b'`')?).into_owned();
            let mut columns = Vec::new();
            if self.peek()? == Some(b' ') {
                self.r.consume(1);
            }
            if self.peek()? == Some(b'(') {
                self.r.consume(1);
                loop {
                    self.expect(b'`')?;
                    columns.push(String::from_utf8_lossy(&self.read_until_byte(b'`')?).into_owned());
                    match self.bump()? {
                        Some(b',') => {
                            if self.peek()? == Some(b' ') {
                                self.r.consume(1);
                            }
                        }
                        Some(b')') => break,
                        _ => return Err(bad("malformed column list")),
                    }
                }
                if self.peek()? == Some(b' ') {
                    self.r.consume(1);
                }
            }
            // "VALUES " then the first "(".
            let word = self.read_until_byte(b' ')?;
            if !word.eq_ignore_ascii_case(b"VALUES") {
                return Err(bad("expected VALUES"));
            }
            return Ok(Some(Header { table, columns }));
        }
    }

    /// The next row of the current statement; `None` once its `;` is read.
    pub fn next_row(&mut self) -> io::Result<Option<Vec<Value>>> {
        match self.peek()? {
            Some(b',') => self.r.consume(1),
            Some(b';') => {
                self.r.consume(1);
                return Ok(None);
            }
            _ => {}
        }
        match self.peek()? {
            Some(b';') => {
                self.r.consume(1);
                return Ok(None);
            }
            None => return Ok(None),
            _ => {}
        }
        self.expect(b'(')?;
        let mut row = Vec::new();
        loop {
            row.push(self.value()?);
            match self.bump()? {
                Some(b',') => {}
                Some(b')') => return Ok(Some(row)),
                other => return Err(bad(&format!("expected , or ) after a value, found {:?}", other.map(|b| b as char)))),
            }
        }
    }

    /// Skips the rest of the current statement without parsing its values.
    pub fn skip_statement(&mut self) -> io::Result<()> {
        loop {
            let buf = self.r.fill_buf()?;
            if buf.is_empty() {
                return Ok(());
            }
            // Fast path: jump to the next quote or semicolon.
            match buf.iter().position(|&b| b == b'\'' || b == b';') {
                None => {
                    let n = buf.len();
                    self.r.consume(n);
                }
                Some(i) => {
                    let found = buf[i];
                    self.r.consume(i + 1);
                    if found == b';' {
                        return Ok(());
                    }
                    self.skip_string_body()?;
                }
            }
        }
    }

    fn value(&mut self) -> io::Result<Value> {
        match self.peek()? {
            None => Err(bad("unexpected end of dump in a row")),
            Some(b'\'') => {
                self.r.consume(1);
                Ok(Value::Bytes(self.string_body()?))
            }
            Some(b'N') => {
                for want in *b"NULL" {
                    self.expect(want)?;
                }
                Ok(Value::Null)
            }
            Some(b'_') => {
                // `_binary '...'` introducer.
                self.read_until_byte(b' ')?;
                self.value()
            }
            Some(b'0') => {
                self.r.consume(1);
                if self.peek()? == Some(b'x') {
                    self.r.consume(1);
                    return Ok(Value::Bytes(self.hex()?));
                }
                self.number(b"0")
            }
            Some(_) => self.number(b""),
        }
    }

    fn hex(&mut self) -> io::Result<Vec<u8>> {
        let mut out = Vec::new();
        let mut high: Option<u8> = None;
        while let Some(b) = self.peek()? {
            let nibble = match b {
                b'0'..=b'9' => b - b'0',
                b'a'..=b'f' => b - b'a' + 10,
                b'A'..=b'F' => b - b'A' + 10,
                _ => break,
            };
            self.r.consume(1);
            match high.take() {
                Some(h) => out.push(h << 4 | nibble),
                None => high = Some(nibble),
            }
        }
        Ok(out)
    }

    fn number(&mut self, prefix: &[u8]) -> io::Result<Value> {
        let mut text = prefix.to_vec();
        while let Some(b) = self.peek()? {
            if matches!(b, b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E') {
                text.push(b);
                self.r.consume(1);
            } else {
                break;
            }
        }
        let s = std::str::from_utf8(&text).map_err(|_| bad("non-ascii number"))?;
        if let Ok(n) = s.parse::<i64>() {
            return Ok(Value::Int(n));
        }
        // Unsigned values past i64::MAX and decimals.
        s.parse::<f64>().map(Value::Float).map_err(|_| bad(&format!("bad number {s:?}")))
    }

    fn skip_string_body(&mut self) -> io::Result<()> {
        loop {
            let buf = self.r.fill_buf()?;
            if buf.is_empty() {
                return Err(bad("unterminated string"));
            }
            match buf.iter().position(|&b| b == b'\'' || b == b'\\') {
                None => {
                    let n = buf.len();
                    self.r.consume(n);
                }
                Some(i) => {
                    let found = buf[i];
                    self.r.consume(i + 1);
                    if found == b'\\' {
                        self.bump()?;
                    } else if self.peek()? == Some(b'\'') {
                        self.r.consume(1); // '' is an escaped quote
                    } else {
                        return Ok(());
                    }
                }
            }
        }
    }

    /// The body of a string whose opening quote was just consumed.
    fn string_body(&mut self) -> io::Result<Vec<u8>> {
        let mut out = Vec::new();
        loop {
            let buf = self.r.fill_buf()?;
            if buf.is_empty() {
                return Err(bad("unterminated string"));
            }
            match buf.iter().position(|&b| b == b'\'' || b == b'\\') {
                None => {
                    out.extend_from_slice(buf);
                    let n = buf.len();
                    self.r.consume(n);
                }
                Some(i) => {
                    out.extend_from_slice(&buf[..i]);
                    let found = buf[i];
                    self.r.consume(i + 1);
                    if found == b'\\' {
                        match self.bump()? {
                            Some(b'0') => out.push(0),
                            Some(b'n') => out.push(b'\n'),
                            Some(b'r') => out.push(b'\r'),
                            Some(b't') => out.push(b'\t'),
                            Some(b'b') => out.push(8),
                            Some(b'Z') => out.push(26),
                            Some(other) => out.push(other),
                            None => return Err(bad("unterminated escape")),
                        }
                    } else if self.peek()? == Some(b'\'') {
                        self.r.consume(1);
                        out.push(b'\'');
                    } else {
                        return Ok(out);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(sql: &str) -> Vec<(Header, Vec<Vec<Value>>)> {
        let mut r = InsertReader::new(sql.as_bytes());
        let mut out = Vec::new();
        while let Some(h) = r.next_insert().unwrap() {
            let mut rows = Vec::new();
            while let Some(row) = r.next_row().unwrap() {
                rows.push(row);
            }
            out.push((h, rows));
        }
        out
    }

    #[test]
    fn parses_extended_inserts_and_skips_other_lines() {
        let sql = "-- comment\nSET x=1;\nINSERT INTO `t` (`a`, `b`, `c`) VALUES (1,'it''s \\'q\\'\\n',NULL),(-2,0x0AFF,3.5);\nCREATE TABLE z;\nINSERT INTO `u` (`x`) VALUES (7);\n";
        let got = read(sql);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].0.table, "t");
        assert_eq!(got[0].0.columns, ["a", "b", "c"]);
        assert_eq!(got[0].1[0], vec![Value::Int(1), Value::Bytes(b"it's 'q'\n".to_vec()), Value::Null]);
        assert_eq!(got[0].1[1], vec![Value::Int(-2), Value::Bytes(vec![0x0a, 0xff]), Value::Float(3.5)]);
        assert_eq!(got[1].1, vec![vec![Value::Int(7)]]);
    }

    #[test]
    fn strings_may_contain_separators_and_semicolons() {
        let got = read("INSERT INTO `t` (`a`) VALUES ('a);(b;'),(0),(0x);\n");
        assert_eq!(got[0].1.len(), 3);
        assert_eq!(got[0].1[0][0], Value::Bytes(b"a);(b;".to_vec()));
        assert_eq!(got[0].1[1][0], Value::Int(0));
        assert_eq!(got[0].1[2][0], Value::Bytes(Vec::new()));
    }

    #[test]
    fn skip_statement_jumps_past_quoted_semicolons() {
        let mut r = InsertReader::new("INSERT INTO `t` (`a`) VALUES ('x;y\\';z'),(2);\nINSERT INTO `u` (`a`) VALUES (9);\n".as_bytes());
        assert_eq!(r.next_insert().unwrap().unwrap().table, "t");
        r.skip_statement().unwrap();
        assert_eq!(r.next_insert().unwrap().unwrap().table, "u");
        assert_eq!(r.next_row().unwrap().unwrap(), vec![Value::Int(9)]);
    }

    #[test]
    fn large_unsigned_values_become_floats() {
        let got = read("INSERT INTO `t` (`a`) VALUES (18446744073709551615);\n");
        assert!(matches!(got[0].1[0][0], Value::Float(_)));
    }
}
