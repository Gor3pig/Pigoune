use std::fs::File;
use std::io::{self, BufReader, Read, Seek};
use std::path::Path;
use std::time::Duration;

use super::frame_timing::FrameTally;
use super::{AnimationTiming, InspectError};

const RIFF_HEADER_LENGTH: i64 = 12;
const EXTENDED_FORMAT: &[u8; 4] = b"VP8X";
const ANIMATION_FRAME: &[u8; 4] = b"ANMF";
const ANIMATION_FLAG: u8 = 0x02;
const EXTENDED_FLAGS_LENGTH: usize = 1;
const FRAME_DURATION_OFFSET: usize = 12;
const FRAME_DURATION_END: usize = 15;

enum Chunk {
    Extended { is_animation: bool },
    Frame { duration: Duration },
    Other,
}

pub fn is_animated(path: &Path) -> Result<bool, InspectError> {
    let file = File::open(path).map_err(|_| InspectError::Unreadable)?;
    Ok(tally_frames(&mut BufReader::new(file), true).is_animation())
}

pub fn timing(path: &Path) -> Option<AnimationTiming> {
    let file = File::open(path).ok()?;
    tally_frames(&mut BufReader::new(file), false).timing()
}

fn tally_frames(reader: &mut (impl Read + Seek), stop_once_animated: bool) -> FrameTally {
    let mut tally = FrameTally::default();
    if reader.seek_relative(RIFF_HEADER_LENGTH).is_err() {
        return tally;
    }
    while let Ok(chunk) = next_chunk(reader) {
        match chunk {
            Chunk::Extended {
                is_animation: false,
            } => break,
            Chunk::Frame { duration } => tally.add(duration),
            Chunk::Extended { is_animation: true } | Chunk::Other => {}
        }
        if stop_once_animated && tally.is_animation() {
            break;
        }
    }
    tally
}

fn next_chunk(reader: &mut (impl Read + Seek)) -> io::Result<Chunk> {
    let mut header = [0; 8];
    reader.read_exact(&mut header)?;
    let kind = [header[0], header[1], header[2], header[3]];
    let length = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
    let needed = match &kind {
        EXTENDED_FORMAT => EXTENDED_FLAGS_LENGTH,
        ANIMATION_FRAME => FRAME_DURATION_END,
        _ => 0,
    };
    let needed_length = u32::try_from(needed).unwrap_or(u32::MAX);
    if length < needed_length {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let mut start = vec![0; needed];
    reader.read_exact(&mut start)?;
    reader.seek_relative(i64::from(length - needed_length) + i64::from(length % 2))?;
    Ok(match &kind {
        EXTENDED_FORMAT => extended(&start),
        ANIMATION_FRAME => frame(&start),
        _ => Chunk::Other,
    })
}

fn extended(start: &[u8]) -> Chunk {
    Chunk::Extended {
        is_animation: start[0] & ANIMATION_FLAG != 0,
    }
}

fn frame(start: &[u8]) -> Chunk {
    let duration = &start[FRAME_DURATION_OFFSET..FRAME_DURATION_END];
    Chunk::Frame {
        duration: Duration::from_millis(u64::from(u32::from_le_bytes([
            duration[0],
            duration[1],
            duration[2],
            0,
        ]))),
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::time::Duration;

    use super::tally_frames;

    fn chunk(kind: [u8; 4], data: &[u8]) -> Vec<u8> {
        let length = u32::try_from(data.len()).expect("small chunk");
        let padding: &[u8] = if data.len() % 2 == 1 { &[0] } else { &[] };
        [&kind, &length.to_le_bytes()[..], data, padding].concat()
    }

    fn extended(flags: u8) -> Vec<u8> {
        chunk(*b"VP8X", &[flags, 0, 0, 0, 0, 0, 0, 0, 0, 0])
    }

    fn frame(milliseconds: u32) -> Vec<u8> {
        let mut data = vec![0; 12];
        data.extend(&milliseconds.to_le_bytes()[..3]);
        data.extend([0, 1, 2, 3]);
        chunk(*b"ANMF", &data)
    }

    fn webp(chunks: &[Vec<u8>]) -> Cursor<Vec<u8>> {
        Cursor::new([b"RIFF\0\0\0\0WEBP".to_vec(), chunks.concat()].concat())
    }

    #[test]
    fn a_simple_webp_has_no_frames() {
        let mut file = webp(&[chunk(*b"VP8L", &[1, 2, 3])]);
        assert_eq!(tally_frames(&mut file, false).timing(), None);
    }

    #[test]
    fn an_extended_webp_without_the_animation_flag_is_still() {
        let mut file = webp(&[extended(0x10), frame(100), frame(100)]);
        assert_eq!(tally_frames(&mut file, false).timing(), None);
    }

    #[test]
    fn every_frame_adds_its_duration() {
        let mut file = webp(&[
            extended(0x12),
            chunk(*b"ANIM", &[0; 6]),
            chunk(*b"ICCP", &[7]),
            frame(70_000),
            frame(5),
            frame(250),
        ]);
        let timing = tally_frames(&mut file, false)
            .timing()
            .expect("frames found");
        assert_eq!(timing.frames, 3);
        assert_eq!(timing.duration, Duration::from_millis(70_000 + 100 + 250));
    }

    #[test]
    fn counting_can_stop_as_soon_as_it_is_an_animation() {
        let mut file = webp(&[extended(0x02), frame(10), frame(10), frame(10)]);
        assert_eq!(
            tally_frames(&mut file, true)
                .timing()
                .map(|timing| timing.frames),
            Some(2)
        );
    }

    #[test]
    fn a_truncated_file_keeps_the_frames_read_so_far() {
        let mut bytes = webp(&[extended(0x02), frame(40)]).into_inner();
        bytes.extend(&frame(40)[..10]);
        assert_eq!(
            tally_frames(&mut Cursor::new(bytes), false)
                .timing()
                .map(|timing| timing.frames),
            Some(1)
        );
    }
}
