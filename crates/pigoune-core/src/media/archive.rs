use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const END_SIGNATURE: [u8; 4] = *b"PK\x05\x06";
const CENTRAL_SIGNATURE: [u8; 4] = *b"PK\x01\x02";
const LOCAL_SIGNATURE: [u8; 4] = *b"PK\x03\x04";
const END_RECORD_LENGTH: usize = 22;
const LARGEST_COMMENT: usize = 65_535;
const CENTRAL_FIXED_LENGTH: usize = 46;
const LOCAL_FIXED_LENGTH: usize = 30;
const MOST_ENTRIES: usize = 100_000;
const LARGEST_DIRECTORY: u64 = 16 * 1024 * 1024;
const METHOD_STORED: u16 = 0;
const METHOD_DEFLATED: u16 = 8;
const ENCRYPTED_FLAG: u16 = 1;
const DATA_DESCRIPTOR_FLAG: u16 = 8;
const ZIP64_COUNT: u16 = u16::MAX;
const ZIP64_FIELD: u32 = u32::MAX;
const EXPANSION_ALLOWANCE: u64 = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ArchiveError {
    #[error("the file is not a readable archive")]
    Unreadable,
    #[error("this kind of archive cannot be read")]
    Unsupported,
    #[error("the archive or one of its files is too large")]
    TooLarge,
    #[error("the archive has no such file")]
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveEntry {
    name: String,
    flags: u16,
    method: u16,
    crc: u32,
    compressed_size: u64,
    size: u64,
    local_offset: u64,
}

impl ArchiveEntry {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn size(&self) -> u64 {
        self.size
    }
}

#[derive(Debug)]
pub struct Archive<R> {
    reader: R,
    length: u64,
    entries: Vec<ArchiveEntry>,
}

impl Archive<File> {
    pub fn open(path: &Path) -> Result<Self, ArchiveError> {
        Self::new(File::open(path).map_err(|_| ArchiveError::Unreadable)?)
    }
}

impl<R: Read + Seek> Archive<R> {
    pub fn new(mut reader: R) -> Result<Self, ArchiveError> {
        let length = reader
            .seek(SeekFrom::End(0))
            .map_err(|_| ArchiveError::Unreadable)?;
        let end = find_end_record(&mut reader, length)?;
        let entries = read_directory(&mut reader, &end)?;
        Ok(Self {
            reader,
            length,
            entries,
        })
    }

    #[must_use]
    pub fn entries(&self) -> &[ArchiveEntry] {
        &self.entries
    }

    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.entries.iter().any(|entry| entry.name == name)
    }

    pub fn read_named(&mut self, name: &str, limit: u64) -> Result<Vec<u8>, ArchiveError> {
        let entry = self
            .entries
            .iter()
            .find(|entry| entry.name == name)
            .cloned()
            .ok_or(ArchiveError::Missing)?;
        self.read(&entry, limit)
    }

    pub fn read(&mut self, entry: &ArchiveEntry, limit: u64) -> Result<Vec<u8>, ArchiveError> {
        check_readable(entry, limit)?;
        let start = self.data_start(entry)?;
        let end = start
            .checked_add(entry.compressed_size)
            .ok_or(ArchiveError::Unreadable)?;
        if end > self.length {
            return Err(ArchiveError::Unreadable);
        }
        let packed = self.read_at(start, entry.compressed_size)?;
        let data = unpack(entry, packed)?;
        if crc32(&data) == entry.crc {
            Ok(data)
        } else {
            Err(ArchiveError::Unreadable)
        }
    }

    fn data_start(&mut self, entry: &ArchiveEntry) -> Result<u64, ArchiveError> {
        let header = self.read_at(entry.local_offset, LOCAL_FIXED_LENGTH as u64)?;
        if header[..4] != LOCAL_SIGNATURE {
            return Err(ArchiveError::Unreadable);
        }
        let name_length = u64::from(little_u16(&header, 26)?);
        let extra_length = u64::from(little_u16(&header, 28)?);
        entry
            .local_offset
            .checked_add(LOCAL_FIXED_LENGTH as u64 + name_length + extra_length)
            .ok_or(ArchiveError::Unreadable)
    }

    fn read_at(&mut self, offset: u64, length: u64) -> Result<Vec<u8>, ArchiveError> {
        read_exactly(&mut self.reader, self.length, offset, length)
    }
}

#[must_use]
pub fn first_stored_text<R: Read + Seek>(reader: &mut R, name: &str, limit: u64) -> Option<String> {
    let length = reader.seek(SeekFrom::End(0)).ok()?;
    let header = read_exactly(reader, length, 0, LOCAL_FIXED_LENGTH as u64).ok()?;
    let flags = little_u16(&header, 6).ok()?;
    let method = little_u16(&header, 8).ok()?;
    let compressed = u64::from(little_u32(&header, 18).ok()?);
    let size = u64::from(little_u32(&header, 22).ok()?);
    let name_length = u64::from(little_u16(&header, 26).ok()?);
    let extra_length = u64::from(little_u16(&header, 28).ok()?);
    let usable = header[..4] == LOCAL_SIGNATURE
        && method == METHOD_STORED
        && flags & (ENCRYPTED_FLAG | DATA_DESCRIPTOR_FLAG) == 0
        && compressed == size
        && size <= limit
        && name_length == u64::try_from(name.len()).ok()?;
    if !usable {
        return None;
    }
    let stored_name = read_exactly(reader, length, LOCAL_FIXED_LENGTH as u64, name_length).ok()?;
    if stored_name != name.as_bytes() {
        return None;
    }
    let data_start = LOCAL_FIXED_LENGTH as u64 + name_length + extra_length;
    let data = read_exactly(reader, length, data_start, size).ok()?;
    String::from_utf8(data).ok()
}

struct EndRecord {
    count: usize,
    directory_offset: u64,
    directory_size: u64,
}

fn find_end_record<R: Read + Seek>(reader: &mut R, length: u64) -> Result<EndRecord, ArchiveError> {
    let window = length.min((END_RECORD_LENGTH + LARGEST_COMMENT) as u64);
    let tail = read_exactly(reader, length, length - window, window)?;
    let position = (0..=tail.len().saturating_sub(END_RECORD_LENGTH))
        .rev()
        .find(|&start| is_end_record(&tail, start))
        .ok_or(ArchiveError::Unreadable)?;
    let record = &tail[position..];
    let disk = little_u16(record, 4)?;
    let directory_disk = little_u16(record, 6)?;
    let count = little_u16(record, 10)?;
    let size = little_u32(record, 12)?;
    let offset = little_u32(record, 16)?;
    if disk != 0 || directory_disk != 0 {
        return Err(ArchiveError::Unsupported);
    }
    if count == ZIP64_COUNT || size == ZIP64_FIELD || offset == ZIP64_FIELD {
        return Err(ArchiveError::Unsupported);
    }
    let end_position =
        length - window + u64::try_from(position).map_err(|_| ArchiveError::Unreadable)?;
    let (size, offset) = (u64::from(size), u64::from(offset));
    if offset
        .checked_add(size)
        .is_none_or(|end| end > end_position)
    {
        return Err(ArchiveError::Unreadable);
    }
    Ok(EndRecord {
        count: usize::from(count),
        directory_offset: offset,
        directory_size: size,
    })
}

fn is_end_record(tail: &[u8], start: usize) -> bool {
    let Some(record) = tail.get(start..start + END_RECORD_LENGTH) else {
        return false;
    };
    let comment_length = usize::from(u16::from_le_bytes([record[20], record[21]]));
    record[..4] == END_SIGNATURE && start + END_RECORD_LENGTH + comment_length == tail.len()
}

fn read_directory<R: Read + Seek>(
    reader: &mut R,
    end: &EndRecord,
) -> Result<Vec<ArchiveEntry>, ArchiveError> {
    if end.count > MOST_ENTRIES || end.directory_size > LARGEST_DIRECTORY {
        return Err(ArchiveError::TooLarge);
    }
    let length = end
        .directory_offset
        .checked_add(end.directory_size)
        .ok_or(ArchiveError::Unreadable)?;
    let directory = read_exactly(reader, length, end.directory_offset, end.directory_size)?;
    let mut entries = Vec::with_capacity(end.count);
    let mut position = 0;
    for _ in 0..end.count {
        let (entry, next) = read_entry(&directory, position)?;
        entries.push(entry);
        position = next;
    }
    Ok(entries)
}

fn read_entry(directory: &[u8], position: usize) -> Result<(ArchiveEntry, usize), ArchiveError> {
    let fixed = directory
        .get(position..position + CENTRAL_FIXED_LENGTH)
        .ok_or(ArchiveError::Unreadable)?;
    if fixed[..4] != CENTRAL_SIGNATURE {
        return Err(ArchiveError::Unreadable);
    }
    let name_length = usize::from(little_u16(fixed, 28)?);
    let extra_length = usize::from(little_u16(fixed, 30)?);
    let comment_length = usize::from(little_u16(fixed, 32)?);
    let name_start = position + CENTRAL_FIXED_LENGTH;
    let name = directory
        .get(name_start..name_start + name_length)
        .ok_or(ArchiveError::Unreadable)?;
    let compressed_size = little_u32(fixed, 20)?;
    let size = little_u32(fixed, 24)?;
    let local_offset = little_u32(fixed, 42)?;
    if [compressed_size, size, local_offset].contains(&ZIP64_FIELD) {
        return Err(ArchiveError::Unsupported);
    }
    let entry = ArchiveEntry {
        name: String::from_utf8_lossy(name).into_owned(),
        flags: little_u16(fixed, 8)?,
        method: little_u16(fixed, 10)?,
        crc: little_u32(fixed, 16)?,
        compressed_size: u64::from(compressed_size),
        size: u64::from(size),
        local_offset: u64::from(local_offset),
    };
    let next = name_start + name_length + extra_length + comment_length;
    Ok((entry, next))
}

fn check_readable(entry: &ArchiveEntry, limit: u64) -> Result<(), ArchiveError> {
    if entry.flags & ENCRYPTED_FLAG != 0 {
        return Err(ArchiveError::Unsupported);
    }
    if entry.method != METHOD_STORED && entry.method != METHOD_DEFLATED {
        return Err(ArchiveError::Unsupported);
    }
    let packed_limit = limit
        .saturating_add(limit / 8)
        .saturating_add(EXPANSION_ALLOWANCE);
    if entry.size > limit || entry.compressed_size > packed_limit {
        return Err(ArchiveError::TooLarge);
    }
    Ok(())
}

fn unpack(entry: &ArchiveEntry, packed: Vec<u8>) -> Result<Vec<u8>, ArchiveError> {
    let size = usize::try_from(entry.size).map_err(|_| ArchiveError::TooLarge)?;
    let data = if entry.method == METHOD_STORED {
        packed
    } else {
        miniz_oxide::inflate::decompress_to_vec_with_limit(&packed, size)
            .map_err(|_| ArchiveError::Unreadable)?
    };
    if data.len() == size {
        Ok(data)
    } else {
        Err(ArchiveError::Unreadable)
    }
}

fn read_exactly<R: Read + Seek>(
    reader: &mut R,
    length: u64,
    offset: u64,
    wanted: u64,
) -> Result<Vec<u8>, ArchiveError> {
    let end = offset.checked_add(wanted).ok_or(ArchiveError::Unreadable)?;
    if end > length {
        return Err(ArchiveError::Unreadable);
    }
    reader
        .seek(SeekFrom::Start(offset))
        .map_err(|_| ArchiveError::Unreadable)?;
    let mut data = Vec::new();
    reader
        .take(wanted)
        .read_to_end(&mut data)
        .map_err(|_| ArchiveError::Unreadable)?;
    if u64::try_from(data.len()).is_ok_and(|read| read == wanted) {
        Ok(data)
    } else {
        Err(ArchiveError::Unreadable)
    }
}

fn little_u16(bytes: &[u8], offset: usize) -> Result<u16, ArchiveError> {
    let field = bytes
        .get(offset..offset + 2)
        .ok_or(ArchiveError::Unreadable)?;
    Ok(u16::from_le_bytes([field[0], field[1]]))
}

fn little_u32(bytes: &[u8], offset: usize) -> Result<u32, ArchiveError> {
    let field = bytes
        .get(offset..offset + 4)
        .ok_or(ArchiveError::Unreadable)?;
    Ok(u32::from_le_bytes([field[0], field[1], field[2], field[3]]))
}

const CRC_TABLE: [u32; 256] = crc_table();

const fn crc_table() -> [u32; 256] {
    let mut table = [0_u32; 256];
    let mut index = 0_u32;
    while index < 256 {
        let mut value = index;
        let mut bit = 0;
        while bit < 8 {
            value = if value & 1 == 1 {
                0xEDB8_8320 ^ (value >> 1)
            } else {
                value >> 1
            };
            bit += 1;
        }
        table[index as usize] = value;
        index += 1;
    }
    table
}

fn crc32(data: &[u8]) -> u32 {
    !data.iter().fold(u32::MAX, |crc, &byte| {
        CRC_TABLE[((crc ^ u32::from(byte)) & 0xFF) as usize] ^ (crc >> 8)
    })
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::{Archive, ArchiveError, crc32, first_stored_text};

    const STORED: u16 = 0;
    const DEFLATED: u16 = 8;

    struct Part<'a> {
        name: &'a str,
        data: &'a [u8],
        method: u16,
    }

    fn stored<'a>(name: &'a str, data: &'a [u8]) -> Part<'a> {
        Part {
            name,
            data,
            method: STORED,
        }
    }

    fn deflated<'a>(name: &'a str, data: &'a [u8]) -> Part<'a> {
        Part {
            name,
            data,
            method: DEFLATED,
        }
    }

    fn zip(parts: &[Part]) -> Vec<u8> {
        let mut file = Vec::new();
        let mut directory = Vec::new();
        for part in parts {
            let packed = if part.method == DEFLATED {
                miniz_oxide::deflate::compress_to_vec(part.data, 6)
            } else {
                part.data.to_vec()
            };
            let offset = u32::try_from(file.len()).unwrap();
            let name_length = u16::try_from(part.name.len()).unwrap();
            let crc = crc32(part.data);
            let packed_length = u32::try_from(packed.len()).unwrap();
            let length = u32::try_from(part.data.len()).unwrap();
            file.extend_from_slice(b"PK\x03\x04");
            file.extend_from_slice(&[20, 0, 0, 0]);
            file.extend_from_slice(&part.method.to_le_bytes());
            file.extend_from_slice(&[0; 4]);
            file.extend_from_slice(&crc.to_le_bytes());
            file.extend_from_slice(&packed_length.to_le_bytes());
            file.extend_from_slice(&length.to_le_bytes());
            file.extend_from_slice(&name_length.to_le_bytes());
            file.extend_from_slice(&[0, 0]);
            file.extend_from_slice(part.name.as_bytes());
            file.extend_from_slice(&packed);
            directory.extend_from_slice(b"PK\x01\x02");
            directory.extend_from_slice(&[20, 0, 20, 0, 0, 0]);
            directory.extend_from_slice(&part.method.to_le_bytes());
            directory.extend_from_slice(&[0; 4]);
            directory.extend_from_slice(&crc.to_le_bytes());
            directory.extend_from_slice(&packed_length.to_le_bytes());
            directory.extend_from_slice(&length.to_le_bytes());
            directory.extend_from_slice(&name_length.to_le_bytes());
            directory.extend_from_slice(&[0; 12]);
            directory.extend_from_slice(&offset.to_le_bytes());
            directory.extend_from_slice(part.name.as_bytes());
        }
        let directory_offset = u32::try_from(file.len()).unwrap();
        let directory_length = u32::try_from(directory.len()).unwrap();
        let count = u16::try_from(parts.len()).unwrap();
        file.extend_from_slice(&directory);
        file.extend_from_slice(b"PK\x05\x06");
        file.extend_from_slice(&[0; 4]);
        file.extend_from_slice(&count.to_le_bytes());
        file.extend_from_slice(&count.to_le_bytes());
        file.extend_from_slice(&directory_length.to_le_bytes());
        file.extend_from_slice(&directory_offset.to_le_bytes());
        file.extend_from_slice(&[0, 0]);
        file
    }

    fn open(bytes: Vec<u8>) -> Result<Archive<Cursor<Vec<u8>>>, ArchiveError> {
        Archive::new(Cursor::new(bytes))
    }

    fn sample() -> Vec<u8> {
        zip(&[
            stored("mimetype", b"image/openraster"),
            deflated("stack.xml", &b"<image/>".repeat(50)),
            stored("data/empty.png", b""),
        ])
    }

    #[test]
    fn lists_and_reads_stored_and_deflated_files() {
        let mut archive = open(sample()).expect("the archive opens");

        let names: Vec<&str> = archive
            .entries()
            .iter()
            .map(super::ArchiveEntry::name)
            .collect();

        assert_eq!(names, ["mimetype", "stack.xml", "data/empty.png"]);
        assert_eq!(
            archive.read_named("mimetype", 100).expect("stored file"),
            b"image/openraster"
        );
        assert_eq!(
            archive
                .read_named("stack.xml", 1000)
                .expect("deflated file"),
            b"<image/>".repeat(50)
        );
        assert_eq!(
            archive.read_named("data/empty.png", 10).expect("empty"),
            b""
        );
        assert!(archive.contains("stack.xml"));
        assert!(!archive.contains("missing"));
    }

    #[test]
    fn a_missing_file_is_reported() {
        let mut archive = open(sample()).expect("the archive opens");

        assert_eq!(
            archive.read_named("absent.png", 10),
            Err(ArchiveError::Missing)
        );
    }

    #[test]
    fn a_file_larger_than_the_limit_is_refused_before_reading() {
        let mut archive = open(sample()).expect("the archive opens");

        assert_eq!(
            archive.read_named("stack.xml", 100),
            Err(ArchiveError::TooLarge)
        );
    }

    #[test]
    fn reads_the_leading_stored_text() {
        let bytes = sample();

        let kind = first_stored_text(&mut Cursor::new(bytes.clone()), "mimetype", 100);
        let other = first_stored_text(&mut Cursor::new(bytes.clone()), "stack.xml", 100);
        let too_small = first_stored_text(&mut Cursor::new(bytes), "mimetype", 4);

        assert_eq!(kind.as_deref(), Some("image/openraster"));
        assert_eq!(other, None);
        assert_eq!(too_small, None);
    }

    #[test]
    fn a_damaged_file_fails_its_checksum() {
        let mut bytes = zip(&[stored("a.txt", b"hello world")]);
        let position = bytes
            .windows(5)
            .position(|window| window == b"hello")
            .expect("the data is in the file");
        bytes[position] = b'J';
        let mut archive = open(bytes).expect("the directory is intact");

        assert_eq!(
            archive.read_named("a.txt", 100),
            Err(ArchiveError::Unreadable)
        );
    }

    #[test]
    fn a_size_that_lies_cannot_swell_the_output() {
        let real = b"x".repeat(5000);
        let mut bytes = zip(&[deflated("bomb.bin", &real)]);
        let declared = 100_u32.to_le_bytes();
        let directory_start = bytes
            .windows(4)
            .position(|window| window == b"PK\x01\x02")
            .expect("the directory exists");
        bytes[directory_start + 24..directory_start + 28].copy_from_slice(&declared);
        let mut archive = open(bytes).expect("the directory is readable");

        assert_eq!(
            archive.read_named("bomb.bin", 10_000),
            Err(ArchiveError::Unreadable)
        );
    }

    #[test]
    fn encrypted_and_large_archives_are_refused() {
        let mut encrypted = zip(&[stored("a.txt", b"hi")]);
        let directory_start = encrypted
            .windows(4)
            .position(|window| window == b"PK\x01\x02")
            .expect("the directory exists");
        encrypted[directory_start + 8] = 1;
        let mut archive = open(encrypted).expect("the directory is readable");
        assert_eq!(
            archive.read_named("a.txt", 10),
            Err(ArchiveError::Unsupported)
        );

        let mut large = zip(&[stored("a.txt", b"hi")]);
        let end_start = large.len() - 22;
        large[end_start + 10..end_start + 12].copy_from_slice(&u16::MAX.to_le_bytes());
        assert_eq!(open(large).err(), Some(ArchiveError::Unsupported));
    }

    #[test]
    fn other_files_are_not_archives() {
        assert_eq!(open(Vec::new()).err(), Some(ArchiveError::Unreadable));
        assert_eq!(
            open(b"GIF89a not a zip".to_vec()).err(),
            Some(ArchiveError::Unreadable)
        );
    }

    #[test]
    fn never_panics_on_truncated_or_damaged_archives() {
        let valid = sample();
        for length in 0..valid.len() {
            let _ = open(valid[..length].to_vec()).map(|mut archive| {
                let names: Vec<String> = archive
                    .entries()
                    .iter()
                    .map(|entry| entry.name().to_owned())
                    .collect();
                for name in names {
                    let _ = archive.read_named(&name, 10_000);
                }
            });
        }
        for position in 0..valid.len() {
            for value in [0, 1, 0x7f, 0x80, 0xff] {
                let mut damaged = valid.clone();
                damaged[position] = value;
                if let Ok(mut archive) = open(damaged) {
                    let names: Vec<String> = archive
                        .entries()
                        .iter()
                        .map(|entry| entry.name().to_owned())
                        .collect();
                    for name in names {
                        let _ = archive.read_named(&name, 10_000);
                        let _ = first_stored_text(&mut Cursor::new(Vec::<u8>::new()), &name, 10);
                    }
                }
            }
        }
    }

    #[test]
    fn reads_archives_written_by_other_programs() {
        for name in ["python-archive.zip", "info-zip-archive.zip"] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(name);
            let mut archive = Archive::open(&path).expect("the archive opens");

            assert_eq!(
                archive.read_named("mimetype", 100).expect("stored file"),
                b"image/openraster",
                "{name}"
            );
            let text = archive
                .read_named("notes.txt", 10_000)
                .expect("deflated file");
            assert!(text.starts_with(b"Pigoune"), "{name}");
            assert_eq!(text.len(), 700, "{name}");
        }
    }
}
