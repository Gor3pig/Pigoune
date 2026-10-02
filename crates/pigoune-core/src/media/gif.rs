use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::Path;

use super::InspectError;

const SCREEN_DESCRIPTOR_END: usize = 13;
const SCREEN_DESCRIPTOR_FLAGS: usize = 10;
const IMAGE_DESCRIPTOR_LENGTH: usize = 9;
const IMAGE_DESCRIPTOR_FLAGS: usize = 8;
const IMAGE_SEPARATOR: u8 = 0x2C;
const EXTENSION_INTRODUCER: u8 = 0x21;
const COLOR_TABLE_PRESENT: u8 = 0x80;
const COLOR_TABLE_SIZE_BITS: u8 = 0x07;
const FRAMES_OF_AN_ANIMATION: usize = 2;

enum Block {
    Image,
    Extension,
    End,
}

pub fn is_animated(path: &Path) -> Result<bool, InspectError> {
    let file = File::open(path).map_err(|_| InspectError::Unreadable)?;
    let frames = count_frames(&mut BufReader::new(file), FRAMES_OF_AN_ANIMATION);
    Ok(frames >= FRAMES_OF_AN_ANIMATION)
}

fn count_frames(reader: &mut impl Read, limit: usize) -> usize {
    if skip_screen_descriptor(reader).is_err() {
        return 0;
    }
    let mut frames = 0;
    while frames < limit {
        match next_block(reader) {
            Ok(Block::Image) => frames += 1,
            Ok(Block::Extension) => {}
            Ok(Block::End) | Err(_) => break,
        }
    }
    frames
}

fn skip_screen_descriptor(reader: &mut impl Read) -> io::Result<()> {
    let descriptor: [u8; SCREEN_DESCRIPTOR_END] = read_array(reader)?;
    skip_color_table(reader, descriptor[SCREEN_DESCRIPTOR_FLAGS])
}

fn next_block(reader: &mut impl Read) -> io::Result<Block> {
    match read_array::<1>(reader)?[0] {
        IMAGE_SEPARATOR => {
            let descriptor: [u8; IMAGE_DESCRIPTOR_LENGTH] = read_array(reader)?;
            skip_color_table(reader, descriptor[IMAGE_DESCRIPTOR_FLAGS])?;
            skip(reader, 1)?;
            skip_sub_blocks(reader)?;
            Ok(Block::Image)
        }
        EXTENSION_INTRODUCER => {
            skip(reader, 1)?;
            skip_sub_blocks(reader)?;
            Ok(Block::Extension)
        }
        _ => Ok(Block::End),
    }
}

fn skip_color_table(reader: &mut impl Read, flags: u8) -> io::Result<()> {
    if flags & COLOR_TABLE_PRESENT == 0 {
        return Ok(());
    }
    skip(reader, 3_u64 << ((flags & COLOR_TABLE_SIZE_BITS) + 1))
}

fn skip_sub_blocks(reader: &mut impl Read) -> io::Result<()> {
    loop {
        let length = read_array::<1>(reader)?[0];
        if length == 0 {
            return Ok(());
        }
        skip(reader, u64::from(length))?;
    }
}

fn skip(reader: &mut impl Read, count: u64) -> io::Result<()> {
    let skipped = io::copy(&mut reader.take(count), &mut io::sink())?;
    if skipped == count {
        Ok(())
    } else {
        Err(io::ErrorKind::UnexpectedEof.into())
    }
}

fn read_array<const LENGTH: usize>(reader: &mut impl Read) -> io::Result<[u8; LENGTH]> {
    let mut bytes = [0; LENGTH];
    reader.read_exact(&mut bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::count_frames;

    const HEADER: &[u8] = b"GIF89a\x01\x00\x01\x00\x80\x00\x00\x00\x00\x00\xff\xff\xff";
    const FRAME: &[u8] = b"\x2c\x00\x00\x00\x00\x01\x00\x01\x00\x00\x02\x02\x44\x01\x00";
    const GRAPHIC_CONTROL: &[u8] = b"\x21\xf9\x04\x00\x0a\x00\x00\x00";
    const TRAILER: &[u8] = b"\x3b";

    fn gif(parts: &[&[u8]]) -> Vec<u8> {
        parts.concat()
    }

    #[test]
    fn a_single_frame_is_counted_once() {
        let bytes = gif(&[HEADER, GRAPHIC_CONTROL, FRAME, TRAILER]);
        assert_eq!(count_frames(&mut bytes.as_slice(), 10), 1);
    }

    #[test]
    fn every_frame_is_counted() {
        let bytes = gif(&[
            HEADER,
            GRAPHIC_CONTROL,
            FRAME,
            GRAPHIC_CONTROL,
            FRAME,
            GRAPHIC_CONTROL,
            FRAME,
            TRAILER,
        ]);
        assert_eq!(count_frames(&mut bytes.as_slice(), 10), 3);
    }

    #[test]
    fn counting_stops_at_the_limit() {
        let bytes = gif(&[HEADER, FRAME, FRAME, FRAME, TRAILER]);
        assert_eq!(count_frames(&mut bytes.as_slice(), 2), 2);
    }

    #[test]
    fn a_truncated_file_keeps_the_frames_read_so_far() {
        let bytes = gif(&[HEADER, FRAME, &FRAME[..6]]);
        assert_eq!(count_frames(&mut bytes.as_slice(), 10), 1);
    }

    #[test]
    fn a_header_alone_has_no_frame() {
        assert_eq!(count_frames(&mut &HEADER[..8], 10), 0);
    }
}
