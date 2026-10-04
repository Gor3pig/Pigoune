use std::fs::File;
use std::io::{self, BufReader, Read, Seek};
use std::path::Path;
use std::time::Duration;

use super::frame_timing::FrameTally;
use super::{AnimationTiming, InspectError};

const SIGNATURE_LENGTH: i64 = 8;
const CHUNK_CHECKSUM_LENGTH: i64 = 4;
const ANIMATION_CONTROL: &[u8; 4] = b"acTL";
const FRAME_CONTROL: &[u8; 4] = b"fcTL";
const IMAGE_DATA: &[u8; 4] = b"IDAT";
const IMAGE_END: &[u8; 4] = b"IEND";
const FRAME_DELAY_OFFSET: usize = 20;
const FRAME_DELAY_END: usize = 24;
const DEFAULT_DELAY_DENOMINATOR: u16 = 100;
const MILLISECONDS_PER_SECOND: u64 = 1000;
const FRAMES_OF_AN_ANIMATION: u32 = 2;

enum Chunk {
    AnimationControl { frames: u32 },
    FrameControl { delay: Duration },
    ImageData,
    Other,
    End,
}

pub fn is_animated(path: &Path) -> Result<bool, InspectError> {
    let file = File::open(path).map_err(|_| InspectError::Unreadable)?;
    Ok(declared_frames(&mut BufReader::new(file))
        .is_some_and(|frames| frames >= FRAMES_OF_AN_ANIMATION))
}

pub fn timing(path: &Path) -> Option<AnimationTiming> {
    let file = File::open(path).ok()?;
    tally_frames(&mut BufReader::new(file)).timing()
}

fn declared_frames(reader: &mut (impl Read + Seek)) -> Option<u32> {
    reader.seek_relative(SIGNATURE_LENGTH).ok()?;
    loop {
        match next_chunk(reader).ok()? {
            Chunk::AnimationControl { frames } => return Some(frames),
            Chunk::ImageData | Chunk::End => return None,
            Chunk::FrameControl { .. } | Chunk::Other => {}
        }
    }
}

fn tally_frames(reader: &mut (impl Read + Seek)) -> FrameTally {
    let mut tally = FrameTally::default();
    if reader.seek_relative(SIGNATURE_LENGTH).is_err() {
        return tally;
    }
    while let Ok(chunk) = next_chunk(reader) {
        match chunk {
            Chunk::FrameControl { delay } => tally.add(delay),
            Chunk::End => break,
            Chunk::AnimationControl { .. } | Chunk::ImageData | Chunk::Other => {}
        }
    }
    tally
}

fn next_chunk(reader: &mut (impl Read + Seek)) -> io::Result<Chunk> {
    let mut header = [0; 8];
    reader.read_exact(&mut header)?;
    let length = u32::from_be_bytes([header[0], header[1], header[2], header[3]]);
    let kind = [header[4], header[5], header[6], header[7]];
    match &kind {
        ANIMATION_CONTROL => animation_control(&read_data(reader, length)?),
        FRAME_CONTROL => frame_control(&read_data(reader, length)?),
        IMAGE_END => Ok(Chunk::End),
        IMAGE_DATA => skip_data(reader, length).map(|()| Chunk::ImageData),
        _ => skip_data(reader, length).map(|()| Chunk::Other),
    }
}

fn animation_control(data: &[u8]) -> io::Result<Chunk> {
    let frames = data.first_chunk::<4>().ok_or(io::ErrorKind::InvalidData)?;
    Ok(Chunk::AnimationControl {
        frames: u32::from_be_bytes(*frames),
    })
}

fn frame_control(data: &[u8]) -> io::Result<Chunk> {
    let delay = data
        .get(FRAME_DELAY_OFFSET..FRAME_DELAY_END)
        .ok_or(io::ErrorKind::InvalidData)?;
    Ok(Chunk::FrameControl {
        delay: frame_delay(
            u16::from_be_bytes([delay[0], delay[1]]),
            u16::from_be_bytes([delay[2], delay[3]]),
        ),
    })
}

fn skip_data(reader: &mut (impl Read + Seek), length: u32) -> io::Result<()> {
    reader.seek_relative(i64::from(length) + CHUNK_CHECKSUM_LENGTH)
}

fn read_data(reader: &mut (impl Read + Seek), length: u32) -> io::Result<Vec<u8>> {
    let mut data = Vec::new();
    reader.take(u64::from(length)).read_to_end(&mut data)?;
    if data.len() != length as usize {
        return Err(io::ErrorKind::UnexpectedEof.into());
    }
    reader.seek_relative(CHUNK_CHECKSUM_LENGTH)?;
    Ok(data)
}

fn frame_delay(numerator: u16, denominator: u16) -> Duration {
    let denominator = if denominator == 0 {
        DEFAULT_DELAY_DENOMINATOR
    } else {
        denominator
    };
    Duration::from_millis(u64::from(numerator) * MILLISECONDS_PER_SECOND / u64::from(denominator))
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::time::Duration;

    use super::{declared_frames, frame_delay, tally_frames};

    const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

    fn chunk(kind: [u8; 4], data: &[u8]) -> Vec<u8> {
        let length = u32::try_from(data.len()).expect("small chunk");
        [&length.to_be_bytes()[..], &kind, data, &[0; 4]].concat()
    }

    fn frame_control(numerator: u16, denominator: u16) -> Vec<u8> {
        let mut data = vec![0; 20];
        data.extend(numerator.to_be_bytes());
        data.extend(denominator.to_be_bytes());
        data.extend([0, 0]);
        chunk(*b"fcTL", &data)
    }

    fn png(chunks: &[Vec<u8>]) -> Cursor<Vec<u8>> {
        Cursor::new([SIGNATURE.to_vec(), chunks.concat()].concat())
    }

    fn header() -> Vec<u8> {
        chunk(*b"IHDR", &[0; 13])
    }

    fn animation_control(frames: u32) -> Vec<u8> {
        chunk(*b"acTL", &[&frames.to_be_bytes()[..], &[0; 4]].concat())
    }

    #[test]
    fn a_still_png_declares_no_frames() {
        let mut file = png(&[header(), chunk(*b"IDAT", &[1, 2, 3]), chunk(*b"IEND", &[])]);
        assert_eq!(declared_frames(&mut file), None);
    }

    #[test]
    fn an_animated_png_declares_its_frames_before_the_image_data() {
        let mut file = png(&[
            header(),
            animation_control(3),
            frame_control(1, 10),
            chunk(*b"IDAT", &[1]),
            chunk(*b"IEND", &[]),
        ]);
        assert_eq!(declared_frames(&mut file), Some(3));
    }

    #[test]
    fn an_animation_control_after_the_image_data_is_ignored() {
        let mut file = png(&[header(), chunk(*b"IDAT", &[1]), animation_control(3)]);
        assert_eq!(declared_frames(&mut file), None);
    }

    #[test]
    fn every_frame_control_adds_its_delay() {
        let mut file = png(&[
            header(),
            animation_control(3),
            frame_control(1, 10),
            chunk(*b"IDAT", &[1]),
            frame_control(50, 1000),
            chunk(*b"fdAT", &[1]),
            frame_control(0, 0),
            chunk(*b"fdAT", &[1]),
            chunk(*b"IEND", &[]),
        ]);
        let timing = tally_frames(&mut file).timing().expect("frames found");
        assert_eq!(timing.frames, 3);
        assert_eq!(timing.duration, Duration::from_millis(100 + 50 + 100));
    }

    #[test]
    fn a_truncated_file_keeps_the_frames_read_so_far() {
        let mut bytes = png(&[header(), animation_control(2), frame_control(1, 5)]).into_inner();
        bytes.extend(&frame_control(1, 5)[..10]);
        assert_eq!(
            tally_frames(&mut Cursor::new(bytes))
                .timing()
                .map(|timing| timing.frames),
            Some(1)
        );
    }

    #[test]
    fn a_missing_denominator_means_hundredths_of_a_second() {
        assert_eq!(frame_delay(7, 0), Duration::from_millis(70));
        assert_eq!(frame_delay(1, 4), Duration::from_millis(250));
    }
}
