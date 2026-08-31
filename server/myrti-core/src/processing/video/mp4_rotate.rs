use std::{io::SeekFrom, os::unix::fs::MetadataExt, path::Path};

use eyre::{Result, eyre};
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, BufReader},
};

const MATRIX_0: [i32; 9] = [0x00010000, 0, 0, 0, 0x00010000, 0, 0, 0, 0x40000000];
const MATRIX_90: [i32; 9] = [0, 0x00010000, 0, -0x00010000, 0, 0, 0, 0, 0x40000000];
const MATRIX_180: [i32; 9] = [-0x00010000, 0, 0, 0, -0x00010000, 0, 0, 0, 0x40000000];
const MATRIX_270: [i32; 9] = [0, -0x00010000, 0, 0x00010000, 0, 0, 0, 0, 0x40000000];

pub async fn copy_mp4_rotation_metadata(src: &Path, dst: &Path) -> Result<()> {
    let (_src_offset, matrix) = video_stream_matrix_offset(src).await?;
    let (dst_offset, _) = video_stream_matrix_offset(dst).await?;
    let mut file = OpenOptions::new().write(true).open(dst).await?;
    file.seek(SeekFrom::Start(dst_offset)).await?;
    for val in matrix {
        file.write_all(&val.to_be_bytes()).await?;
    }
    Ok(())
}

// rudimentary mp4 parser to find the transform matrix of the first video track.
// no existing tool can easily edit that inplace without modifying anything else, so we do it by hand.
async fn video_stream_matrix_offset(path: &Path) -> Result<(u64, &'static [i32; 9])> {
    let file = File::open(path).await?;
    let mut data = BufReader::new(file);
    let file_size = tokio::fs::metadata(path).await?.size();

    let mut matrix_offset: Option<(u64, &'static [i32; 9])> = None;
    // which box(es) we're currently in
    let mut stack = vec![file_size];

    // should be enough iterations
    for _ in 0..1000 {
        let pos = data.stream_position().await?;
        // Pop boxes that we've already walked past
        while pos >= *stack.last().expect("not empty") {
            stack.pop();
            if stack.is_empty() {
                break;
            }
        }
        // Box starts with u32 size. 1 means large box, 0 means until end of file
        let size_maybe = {
            let mut b = [0u8; 4];
            data.read_exact(&mut b).await?;
            u32::from_be_bytes(b)
        };
        let kind = {
            let mut b = [0u8; 4];
            data.read_exact(&mut b).await?;
            b
        };
        let size: u64 = match size_maybe {
            0 => stack.last().expect("not empty") - pos,
            1 => {
                let mut b = [0u8; 8];
                data.read_exact(&mut b).await?;
                u64::from_be_bytes(b)
            }
            s => s.into(),
        };
        let end = pos + size;
        if end > file_size {
            return Err(eyre!(
                "broken mp4 file: box end {} is past file size {}",
                end,
                file_size
            ));
        }
        if kind.iter().any(|c| !c.is_ascii_alphanumeric()) {
            return Err(eyre!(
                "broken mp4 file: invalid box type {} ({:x}, {:x}, {:x}, {:x})",
                String::from_utf8_lossy(&kind),
                kind[0],
                kind[1],
                kind[2],
                kind[3]
            ));
        }
        stack.push(end);

        // trak
        // | tkhd <- contains transform matrix
        // | mdia
        //   | hdlr <- specifies video or audio track
        match &kind {
            b"hdlr" => {
                data.seek(SeekFrom::Current(
                    1 // version
                        + 3 // flags
                        + 4, // predefined
                ))
                .await?;
                let hdlr = {
                    let mut b = [0u8; 4];
                    data.read_exact(&mut b).await?;
                    b
                };
                if &hdlr == b"vide"
                    && let Some(matrix_offset) = matrix_offset
                {
                    return Ok(matrix_offset);
                }
                data.seek(SeekFrom::Start(end)).await?;
            }
            b"tkhd" => {
                let mut version = [0; 1];
                data.read_exact(&mut version).await?;
                if version == [0] {
                    data.seek(SeekFrom::Current(
                        3 // flags 
                        + 4 // creation
                        + 4 // modification
                        + 4 // trackid
                        + 4 // reserved
                        + 4 // duration
                        + 8 // reserved
                        + 2 // layer
                        + 2 // alt_group
                        + 2 // volume
                        + 2, // reserved
                    ))
                    .await?;
                } else if version == [1] {
                    data.seek(SeekFrom::Current(
                        3 // flags 
                        + 8 // creation
                        + 8 // modification
                        + 4 // trackid
                        + 4 // reserved
                        + 8 // duration
                        + 8 // reserved
                        + 2 // layer
                        + 2 // alt_group
                        + 2 // volume
                        + 2, // reserved
                    ))
                    .await?;
                } else {
                    return Err(eyre!("invalid tkhd version {}", version[0]));
                }
                let mx_bytes = {
                    let mut b = [0u8; 9 * 4];
                    data.read_exact(&mut b).await?;
                    b
                };
                let mut matrix = [0i32; 9];
                for i in 0..9 {
                    matrix[i] = i32::from_be_bytes([
                        mx_bytes[i * 4],
                        mx_bytes[i * 4 + 1],
                        mx_bytes[i * 4 + 2],
                        mx_bytes[i * 4 + 3],
                    ])
                }
                let which_matrix = match matrix {
                    MATRIX_0 => &MATRIX_0,
                    MATRIX_90 => &MATRIX_90,
                    MATRIX_180 => &MATRIX_180,
                    MATRIX_270 => &MATRIX_270,
                    _ => return Err(eyre!("invalid rotation matrix {:?}", matrix)),
                };
                matrix_offset = Some((data.stream_position().await? - 9 * 4, which_matrix));

                data.seek(SeekFrom::Start(end)).await?;
            }
            b"moov" | b"trak" | b"mdia" => { // keep walking, enter the box
            }
            _ => {
                // skip
                data.seek(SeekFrom::Start(end)).await?;
            }
        }
    }
    Err(eyre!("did not find video track and its transform matrix"))
}
