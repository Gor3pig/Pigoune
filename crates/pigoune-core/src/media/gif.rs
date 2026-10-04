use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::Path;
use std::time::Duration;

use super::frame_timing::FrameTally;
use super::{AnimationTiming, InspectError};

const SCREEN_DESCRIPTOR_END: usize = 13;
const SCREEN_DESCRIPTOR_FLAGS: usize = 10;
const IMAGE_DESCRIPTOR_LENGTH: usize = 9;
const IMAGE_DESCRIPTOR_FLAGS: usize = 8;
const IMAGE_SEPARATOR: u8 = 0x2C;
const EXTENSION_INTRODUCER: u8 = 0x21;
const COLOR_TABLE_PRESENT: u8 = 0x80;
const COLOR_TABLE_SIZE_BITS: u8 = 0x07;
const GRAPHIC_CONTROL_LABEL: u8 = 0xF9;
const GRAPHIC_CONTROL_LENGTH: usize = 4;
const FRAMES_OF_AN_ANIMATION: usize = 2;
const MILLISECONDS_PER_DELAY_UNIT: u64 = 10;

enum Block {
    Image,
    Delay(u16),
    Extension,
    End,
}

pub fn is_animated(path: &Path) -> Result<bool, InspectError> {
    let file = File::open(path).map_err(|_| InspectError::Unreadable)?;
    let frames = count_frames(&mut BufReader::new(file), FRAMES_OF_AN_ANIMATION);
    Ok(frames >= FRAMES_OF_AN_ANIMATION)
}

pub fn timing(path: &Path) -> Option<AnimationTiming> {
    let file = File::open(path).ok()?;
    let mut tally = FrameTally::default();
    scan_frames(&mut BufReader::new(file), usize::MAX, |delay| {
        tally.add(Duration::from_millis(
            u64::from(delay) * MILLISECONDS_PER_DELAY_UNIT,
        ));
    });
    tally.timing()
}

fn count_frames(reader: &mut impl Read, limit: usize) -> usize {
    scan_frames(reader, limit, |_| {})
}

fn scan_frames(reader: &mut impl Read, limit: usize, mut on_frame: impl FnMut(u16)) -> usize {
    if skip_screen_descriptor(reader).is_err() {
        return 0;
    }
    let mut frames = 0;
    let mut pending_delay = 0;
    while frames < limit {
        match next_block(reader) {
            Ok(Block::Image) => {
                frames += 1;
                on_frame(pending_delay);
                pending_delay = 0;
            }
            Ok(Block::Delay(delay)) => pending_delay = delay,
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
            let label = read_array::<1>(reader)?[0];
            if label == GRAPHIC_CONTROL_LABEL {
                return read_graphic_control(reader);
            }
            skip_sub_blocks(reader)?;
            Ok(Block::Extension)
        }
        _ => Ok(Block::End),
    }
}

fn read_graphic_control(reader: &mut impl Read) -> io::Result<Block> {
    let length = usize::from(read_array::<1>(reader)?[0]);
    if length < GRAPHIC_CONTROL_LENGTH {
        skip(reader, length as u64)?;
        skip_sub_blocks(reader)?;
        return Ok(Block::Extension);
    }
    let control: [u8; GRAPHIC_CONTROL_LENGTH] = read_array(reader)?;
    skip(reader, (length - GRAPHIC_CONTROL_LENGTH) as u64)?;
    skip_sub_blocks(reader)?;
    Ok(Block::Delay(u16::from_le_bytes([control[1], control[2]])))
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
    use super::{count_frames, scan_frames};

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
    fn each_frame_reports_its_own_delay() {
        let slow = b"\x21\xf9\x04\x00\x32\x00\x00\x00";
        let bytes = gif(&[HEADER, GRAPHIC_CONTROL, FRAME, slow, FRAME, FRAME, TRAILER]);
        let mut delays = Vec::new();
        let frames = scan_frames(&mut bytes.as_slice(), usize::MAX, |delay| {
            delays.push(delay);
        });
        assert_eq!(frames, 3);
        assert_eq!(delays, [10, 50, 0]);
    }

    #[test]
    fn a_header_alone_has_no_frame() {
        assert_eq!(count_frames(&mut &HEADER[..8], 10), 0);
    }
}
