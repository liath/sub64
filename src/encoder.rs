use base64::{engine::general_purpose::STANDARD, Engine as _};
use core::fmt::{self, Debug};
use std::cmp::min;
use std::io::{self, Error, ErrorKind, Read, Seek, SeekFrom};

trait ReadSeek: Read + Seek {}
impl<T: Read + Seek> ReadSeek for T {}

// exclude b64_buffer as it's uselessly large
impl Debug for dyn ReadSeek {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ReadSeek")
    }
}

#[derive(Debug)]
pub struct Encoder {
    inner: Box<dyn ReadSeek>,
    len: usize,
    pos: u64,
}

impl Encoder {
    pub fn new<T: Read + Seek + 'static>(inner: T) -> Encoder {
        let mut e = Encoder {
            inner: Box::new(inner),
            len: 0,
            pos: 0,
        };
        let _ = e.len();
        e
    }

    pub fn len(&mut self) -> usize {
        let inner_len = self.inner.seek(SeekFrom::End(0)).expect("") as usize;
        self.inner.rewind().expect("");
        self.pos = 0;
        self.len = (inner_len as f64 / 3.0).ceil() as usize * 4;
        println!("s64 | len: {} from inner_len: {}", self.len, inner_len);
        self.len
    }

    pub fn is_empty(&mut self) -> bool {
        self.len > 0
    }
}

//
impl Read for Encoder {
    fn read(&mut self, dst: &mut [u8]) -> io::Result<usize> {
        let want = dst.len();
        // inner position is clamped to triplets
        let inner_pos = self.pos / 4 * 3;
        /* println!(
            "s64 | want: {}, have: {}",
            want,
            self.len - (inner_pos / 3 * 4) as usize
        ); */
        self.inner.seek(SeekFrom::Start(inner_pos))?;
        // outer position is clamped to quartets so use the distance to the
        // nearest quartet boundary to tell how many output chars to skip
        let skip = (self.pos - (inner_pos / 3 * 4)) as usize;

        let mut buf = [0; 1024];
        let got = self.inner.read(&mut buf)?;

        // println!("s64 | buf: {:?}, got: {:?}, skip: {}", &buf[..got], got, skip);
        let encoded = STANDARD.encode(&buf[..got]);
        // println!("s64 | encoded: {:?}", encoded);

        // take up to whichever is smaller: the output buffer length or encoded
        // bytes available length
        let take = min(encoded.len(), want + skip);
        let advance = take - skip;
        dst[0..advance].copy_from_slice((encoded)[skip..take].as_bytes());
        self.pos += advance as u64;
        println!("s64 | read: {}", advance);
        Ok(advance)
    }
}

impl Seek for Encoder {
    fn seek(&mut self, style: SeekFrom) -> io::Result<u64> {
        let (base_pos, offset) = match style {
            SeekFrom::Start(n) => {
                self.pos = n;
                println!("s64 | seeking to: {}", n);
                return Ok(n);
            }
            SeekFrom::End(n) => (self.len, n),
            SeekFrom::Current(n) => (self.pos as usize, n),
        };
        match base_pos.checked_add_signed(offset as isize) {
            Some(n) => {
                self.pos = n as u64;
                println!("s64 | seeking to: {}", n);
                Ok(self.pos)
            }
            None => Err(Error::new(
                ErrorKind::InvalidInput,
                "invalid seek to a negative or overflowing position",
            )),
        }
    }
}

#[cfg(test)]
mod cli {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn basic() {
        let source = Cursor::new(b"MEOWMEOW FUZZYFACE.");
        println!("s64 | source: {:?}", source);
        let mut encoder = Encoder::new(source);

        // read the output at different chunk sizes to hopefully ensure things
        // work outside of the happy path
        let mut out1 = [0; 6];
        let mut len = encoder.read(&mut out1).unwrap();
        assert_eq!(std::str::from_utf8(&out1[..len]).expect(""), "TUVPV0");

        let mut out2 = [0; 7];
        len = encoder.read(&mut out2).unwrap();
        assert_eq!(std::str::from_utf8(&out2[..len]).expect(""), "1FT1cgR");

        let mut out3 = [0; 8];
        len = encoder.read(&mut out3).unwrap();
        assert_eq!(std::str::from_utf8(&out3[..len]).expect(""), "lVaWllGQ");

        let mut out4 = [0; 9];
        len = encoder.read(&mut out4).unwrap();
        assert_eq!(std::str::from_utf8(&out4[..len]).expect(""), "UNFLg==");

        // now read the whole thing
        encoder.seek(SeekFrom::Start(0)).expect("");
        let mut out = [0; 26];
        encoder.read_exact(&mut out).unwrap();
        assert_eq!(
            "TUVPV01FT1cgRlVaWllGQUNFLg",
            std::str::from_utf8(&out).expect(""),
        );
    }
}
