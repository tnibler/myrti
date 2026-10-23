use std::{borrow::Cow, fmt::Display, io::SeekFrom, path::Path, time::Duration};

use itertools::Itertools;
use mp4_atom::{
    AsyncReadAtom, AsyncReadFrom, AsyncWriteTo, Atom, Avc1, Avcc, Encode, FourCC, Ftyp, Header,
    Hvc1, Hvcc, Mehd, Mfhd, Moof, Moov, Mp4a, Mvex, Stbl, StszSamples, Styp, Tfdt, Tfhd, Traf,
    Trak, Trex, Trun, TrunEntry, TrunSamples,
};
use rkyv::{Archive, Deserialize, Serialize};
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, BufReader, BufWriter},
};
use tracing::debug;

use super::mpd;

#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum DashIndexError {
    #[error("Error parsing MP4: {0}")]
    ParseError(Cow<'static, str>),
    #[error("Unknown or invalid codec config: {0}")]
    BadCodecConfig(Cow<'static, str>),
    #[error("Sample at given index not found")]
    SampleNotFound,
    #[error("Integer overflow")]
    IntOverflow,
    #[error("Error parsing MP4 atom")]
    AtomParseError(#[from] mp4_atom::Error),
    #[error("Segment does not exist")]
    SegmentIndexOutOfBounds,
    #[error("I/O error")]
    IOError(#[from] std::io::Error),
    #[error("Error writing m4s segment: {0}")]
    SegmentWriteError(Cow<'static, str>),
    #[error("Error serializing index")]
    SerializationError(#[source] Box<dyn std::error::Error + Send + Sync>),
    #[error("Error deserializing index")]
    DeserializationError(#[source] Box<dyn std::error::Error + Send + Sync>),
    #[error("Index file is empty or corrupt")]
    CorruptIndex,
    #[error("Fragmented MP4 is not supported")]
    NotSupported,
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct DiskSegmentIndex {
    header: Vec<u8>,
    samples: SampleRuns,
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct DiskIndex {
    file_prefix: String,
    segments_video: Vec<DiskSegmentIndex>,
    segments_audio: Vec<DiskSegmentIndex>,
}

#[derive(Debug, Clone)]
struct Track {
    track_id: u32,
    timebase: u32,
    segments: Vec<Segment>,
    codec: String,
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
enum SampleRunsInner {
    S32(Vec<(u32, u32)>),
    S64(Vec<(u64, u32)>),
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct SampleRuns {
    inner: SampleRunsInner,
}

impl SampleRuns {
    pub fn new() -> Self {
        Self {
            inner: SampleRunsInner::S32(Default::default()),
        }
    }

    pub fn append(&mut self, offset: u64, len: u32) -> Result<(), DashIndexError> {
        let end: u64 = offset.try_add(len.into())?;
        let offset_32 = u32::try_from(offset);
        let end_32 = u32::try_from(end);
        if let SampleRunsInner::S32(runs) = &mut self.inner
            && end_32.is_err()
        {
            self.inner = SampleRunsInner::S64(
                std::mem::take(runs)
                    .into_iter()
                    .map(|(offset, len)| (u64::from(offset), len))
                    .collect(),
            );
        }
        match &mut self.inner {
            SampleRunsInner::S64(runs) => {
                if let Some((last_offset, last_len)) = runs.last_mut() {
                    // if u64 overflows, fail
                    let prev_end = last_offset.try_add(u64::from(*last_len))?;
                    // if u32 len overflows, add new run instead of extending the last one
                    if prev_end == offset
                        && let Ok(new_len) = last_len.try_add(len)
                    {
                        *last_len = new_len;
                        return Ok(());
                    }
                }
                runs.push((offset, len));
            }
            SampleRunsInner::S32(runs) => {
                let offset_32 = offset_32.expect("checked for overflow");
                match runs.last_mut() {
                    Some((last_offset, last_len))
                        if last_offset.try_add(*last_len)? == offset_32 =>
                    {
                        *last_len = last_len.try_add(len)?;
                    }
                    _ => {
                        runs.push((offset_32, len));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn total_len(&self) -> Result<u64, DashIndexError> {
        match &self.inner {
            SampleRunsInner::S32(runs) => runs
                .iter()
                .map(|(_, len)| u64::from(*len))
                .try_fold(0u64, |acc, len| acc.try_add(len)),
            SampleRunsInner::S64(runs) => runs
                .iter()
                .map(|(_, len)| u64::from(*len))
                .try_fold(0u64, |acc, len| acc.try_add(len)),
        }
    }
}

fn is_key_frame(sample_idx: u32, trak: &Trak) -> bool {
    if let Some(stss) = trak.mdia.minf.stbl.stss.as_ref() {
        stss.entries.binary_search(&sample_idx).is_ok()
    } else {
        true
    }
}

#[derive(Debug, Clone)]
struct Segment {
    start_pts: i64,
    start_dts: u64,
    duration: u64,
    samples: SampleRuns,
    trun: Vec<TrunEntry>,
}

#[derive(Debug, Clone, Copy)]
struct SampleChunk {
    first_sample: u32,
    n_samples: u32,
    offset: u64,
}

#[derive(Debug, Clone)]
struct VideoTrack {
    base: Track,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone)]
struct AudioTrack {
    base: Track,
}

struct SampleOffsets {
    chunks: Vec<SampleChunk>,
    sample_sizes: Vec<u32>,
}

impl SampleOffsets {
    pub fn new(stbl: &Stbl) -> Result<Self, DashIndexError> {
        let chunk_offsets: Vec<u64> = if let Some(co64) = stbl.co64.as_ref() {
            co64.entries.clone()
        } else if let Some(stco) = stbl.stco.as_ref() {
            stco.entries.iter().map(|n| u64::from(*n)).collect()
        } else {
            return Err(DashIndexError::ParseError(
                "no co64 or stco box found".into(),
            ));
        };
        let sample_sizes = match &stbl.stsz.samples {
            StszSamples::Identical { count, size } => {
                vec![*size; count.as_usize()]
            }
            StszSamples::Different { sizes } => sizes.clone(),
        };
        let mut chunks = Vec::default();
        let n_chunks = chunk_offsets.len();
        for (i, stsc_entry) in stbl.stsc.entries.iter().enumerate() {
            let run_len = stbl
                .stsc
                .entries
                .get(i + 1)
                .as_ref()
                .map(|c| c.first_chunk)
                .unwrap_or((n_chunks + 1).try_into().unwrap())
                - stsc_entry.first_chunk;
            for _ in 0..run_len.as_usize() {
                let first_sample = chunks
                    .last()
                    .map(|c: &SampleChunk| c.n_samples.try_add(c.first_sample))
                    .transpose()?
                    .unwrap_or_default();
                let n_samples = stsc_entry.samples_per_chunk;
                if first_sample.try_add(n_samples)?.as_usize() > sample_sizes.len() {
                    return Err(DashIndexError::ParseError(
                        "stsc entry lists more samples than in stsz".into(),
                    ));
                }
                let chunk_offset =
                    *chunk_offsets
                        .get(chunks.len())
                        .ok_or(DashIndexError::ParseError(
                            "stsc entry lists chunks than stco/co64".into(),
                        ))?;
                chunks.push(SampleChunk {
                    first_sample,
                    n_samples,
                    offset: chunk_offset,
                });
            }
        }
        if chunk_offsets.len() != chunks.len() {
            return Err(DashIndexError::ParseError(
                "Differing number of chunks in stco/co64 and stsc".into(),
            ));
        }
        if sample_sizes.len() != chunks.iter().map(|c| c.n_samples.as_usize()).sum::<usize>() {
            return Err(DashIndexError::ParseError(
                "Differing numbers of samples in stsz and stsc".into(),
            ));
        }
        Ok(Self {
            chunks,
            sample_sizes,
        })
    }

    pub fn sample_offset_size(&self, idx: u32) -> Result<(u64, u32), DashIndexError> {
        let idx_s = idx.as_usize();
        if idx_s >= self.sample_sizes.len() {
            return Err(DashIndexError::SampleNotFound);
        }
        let ch = self
            .chunks
            .iter()
            .rev()
            .find(|ch| ch.first_sample <= idx)
            .ok_or(DashIndexError::SampleNotFound)?;

        let offset_in_chunk: u32 = self.sample_sizes[ch.first_sample.as_usize()..idx_s]
            .iter()
            .copied()
            .sum();
        Ok((
            ch.offset.try_add(offset_in_chunk.into())?,
            self.sample_sizes[idx_s],
        ))
    }
}

fn make_init_segment(moov: &Moov, trak: &Trak) -> Result<Moov, DashIndexError> {
    let mut moov_out = moov.clone();
    let mut trak_out = trak.clone();
    moov_out.udta = None;
    moov_out.mvex = Some(Mvex {
        mehd: Some(Mehd {
            fragment_duration: trak.tkhd.duration,
        }),
        trex: vec![Trex {
            track_id: trak.tkhd.track_id,
            default_sample_description_index: trak
                .mdia
                .minf
                .stbl
                .stsc
                .entries
                .first()
                .ok_or_else(|| {
                    DashIndexError::ParseError(
                        format!("track {} stsc has 0 entries", trak.tkhd.track_id).into(),
                    )
                })?
                .sample_description_index,
            default_sample_duration: 0,
            default_sample_size: 0,
            default_sample_flags: 0x10000,
        }],
    });
    moov_out.mvhd.duration = 0;
    trak_out.tkhd.duration = 0;
    trak_out.mdia.mdhd.duration = 0;
    let stbl = &mut trak_out.mdia.minf.stbl;
    stbl.stts.entries.clear();
    stbl.stss = None;
    stbl.stsz = Default::default();
    // must have an (empty) stco
    stbl.stco = Some(Default::default());
    stbl.co64 = None;
    stbl.ctts = None;
    stbl.stsc = Default::default();
    moov_out.trak = vec![trak_out];
    Ok(moov_out)
}

fn hvc_codec_string(hvcc: &Hvcc) -> Result<String, DashIndexError> {
    let profile_space = match hvcc.general_profile_space {
        0 => "",
        1 => "A",
        2 => "B",
        3 => "C",
        _ => {
            return Err(DashIndexError::BadCodecConfig(
                "invalid hvc1 profile space".into(),
            ));
        }
    };
    let _profile = if hvcc.general_profile_idc == 1
        || hvcc.general_profile_compatibility_flags[0] & 1 != 0
    {
        "1" //main
    } else if hvcc.general_profile_idc == 2 || hvcc.general_profile_compatibility_flags[0] & 2 != 0
    {
        "2" // main 10
    } else if hvcc.general_profile_idc == 3 || hvcc.general_profile_compatibility_flags[0] & 4 != 0
    {
        "3" // main still picture
    } else {
        return Err(DashIndexError::BadCodecConfig(
            "invalid hvc1 profile".into(),
        ));
    };
    let tier = match hvcc.general_tier_flag {
        false => "L",
        true => "H",
    };
    let mut mime = format!(
        "hvc1.{}{}.{:x}.{}{}",
        profile_space,
        hvcc.general_profile_idc,
        u32::from_be_bytes(hvcc.general_profile_compatibility_flags).reverse_bits(),
        tier,
        hvcc.general_level_idc,
    );
    for (_, flag) in hvcc
        .general_constraint_indicator_flags
        .iter()
        .enumerate()
        .rev()
        .skip_while(|(idx, f)| *idx != 0 && **f == 0)
    {
        mime.push_str(&format!(".{:x}", flag));
    }
    Ok(mime)
}

fn avc_codec_string(avcc: &Avcc) -> Result<String, DashIndexError> {
    Ok(format!(
        "avc1.{:02x}{:02x}{:02x}",
        avcc.avc_profile_indication, avcc.profile_compatibility, avcc.avc_level_indication
    ))
}

fn mp4a_codec_string(mp4a: &Mp4a) -> Result<String, DashIndexError> {
    if let Some(dec_specific) = mp4a.esds.es_desc.dec_config.dec_specific.as_ref() {
        Ok(format!(
            "mp4a.{:02x}.{}",
            mp4a.esds.es_desc.dec_config.object_type_indication, dec_specific.profile
        ))
    } else {
        Ok(format!(
            "mp4a.{:02x}",
            mp4a.esds.es_desc.dec_config.object_type_indication
        ))
    }
}

fn index_track(trak: &Trak, segment_duration: f32) -> Result<Track, DashIndexError> {
    let stbl = &trak.mdia.minf.stbl;
    let codec = match stbl
        .stsd
        .codecs
        .first()
        .ok_or(DashIndexError::ParseError("no codec in stsd box".into()))?
    {
        mp4_atom::Codec::Hvc1(Hvc1 { hvcc, .. }) => hvc_codec_string(hvcc)?,
        mp4_atom::Codec::Avc1(Avc1 { avcc, .. }) => avc_codec_string(avcc)?,
        mp4_atom::Codec::Mp4a(mp4a) => mp4a_codec_string(mp4a)?,
        mp4_atom::Codec::Twos(_) => String::from("twos"),
        other => {
            return Err(DashIndexError::BadCodecConfig(
                format!("unsupported codec: {:?}", other).into(),
            ));
        }
    };

    let sample_offsets = SampleOffsets::new(stbl)?;
    let sample_durations: Vec<u32> = stbl
        .stts
        .entries
        .iter()
        .flat_map(|entry| std::iter::repeat_n(entry.sample_delta, entry.sample_count.as_usize()))
        .collect();
    if sample_durations.len() != sample_offsets.sample_sizes.len() {
        return Err(DashIndexError::ParseError(
            "Differing numbers of samples in stsz and stts".into(),
        ));
    }
    let sample_dts: Vec<u32> =
        sample_durations
            .iter()
            .dropping_back(1)
            .try_fold(vec![0u32], |mut acc, dt| {
                acc.push(acc.last().unwrap().try_add(*dt)?);
                Ok::<_, DashIndexError>(acc)
            })?;
    let (cts_offsets, sample_pts) = {
        let mut sample_pts: Vec<i64> = sample_dts.iter().copied().map(i64::from).collect();
        if let Some(ctts) = &stbl.ctts {
            let mut times_iter = sample_pts.iter_mut();
            let mut cts_offsets_ = Vec::default();
            for entry in &ctts.entries {
                for t in (&mut times_iter).take(entry.sample_count.as_usize()) {
                    *t = t.try_add(entry.sample_offset)?;
                }
                cts_offsets_.extend(std::iter::repeat_n(
                    entry.sample_offset,
                    entry.sample_count.as_usize(),
                ));
            }
            if cts_offsets_.len() != sample_pts.len() {
                return Err(DashIndexError::ParseError(
                    "Differing numbers of samples in ctts and stts".into(),
                ));
            }
            (Some(cts_offsets_), sample_pts)
        } else {
            (None, sample_pts)
        }
    };

    let tk_timebase = trak.mdia.mdhd.timescale;

    let n_samples = match &stbl.stsz.samples {
        StszSamples::Identical { count, size: _ } => count.as_usize(),
        StszSamples::Different { sizes } => sizes.len(),
    };
    let segment_len_tb = (segment_duration * tk_timebase as f32).round() as u64;
    let mut last_seg_pts: Option<i64> = None;

    let mut new_segment = Segment {
        start_pts: sample_pts[0],
        start_dts: 0,
        duration: 0,
        samples: SampleRuns::new(),
        trun: Default::default(),
    };
    let mut segments: Vec<Segment> = Vec::default();
    for i in 0..n_samples {
        let cts_offset: Option<i64> = cts_offsets.as_ref().map(|offsets| offsets[i]);
        let (offset, size): (u64, u32) =
            sample_offsets.sample_offset_size(i.try_into().unwrap())?;
        let trun_entry = TrunEntry {
            duration: Some(sample_durations[i]),
            size: Some(size),
            cts: cts_offset,
            ..Default::default()
        };
        let pts = sample_pts[i];
        let start_segment = is_key_frame(i.try_into().unwrap(), trak)
            && if let Some(prev_pts) = last_seg_pts {
                let since_last = (pts - prev_pts) as f32;
                since_last > 0.9 * segment_len_tb as f32
            } else {
                false
            };
        if start_segment && !new_segment.trun.is_empty() {
            new_segment.duration = u64::try_from(pts - new_segment.start_pts).map_err(|_| {
                DashIndexError::ParseError("Segment start PTS are not increasing".into())
            })?;
            segments.push(new_segment);
            new_segment = Segment {
                start_pts: pts,
                start_dts: sample_dts[i].into(),
                duration: 0,
                samples: SampleRuns::new(),
                trun: Default::default(),
            };
            last_seg_pts = Some(pts);
        }
        if last_seg_pts.is_none() {
            last_seg_pts = Some(pts);
        }
        new_segment.trun.push(trun_entry);
        new_segment.samples.append(offset, size)?;
    }
    if !new_segment.trun.is_empty() {
        new_segment.duration = u64::try_from(sample_pts.last().unwrap() - new_segment.start_pts)
            .map_err(|_| {
                DashIndexError::ParseError("Segment start PTS are not increasing".into())
            })?;
        segments.push(new_segment);
    }
    Ok(Track {
        track_id: trak.tkhd.track_id,
        timebase: tk_timebase,
        segments,
        codec,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackType {
    Video,
    Audio,
}

impl Display for TrackType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrackType::Video => write!(f, "video"),
            TrackType::Audio => write!(f, "audio"),
        }
    }
}

pub async fn write_segment(
    index: &DeserializedIndex,
    track: TrackType,
    seg_idx: usize,
    in_path: &Path,
    out_dir: &Path,
) -> Result<(), DashIndexError> {
    let segment_index = match track {
        TrackType::Video => index
            .access()
            .segments_video
            .get(seg_idx)
            .ok_or(DashIndexError::SegmentIndexOutOfBounds)?,
        TrackType::Audio => index
            .access()
            .segments_audio
            .get(seg_idx)
            .ok_or(DashIndexError::SegmentIndexOutOfBounds)?,
    };
    let out_path = out_dir.join(format!(
        "{}-{track}-{seg_idx}.m4s",
        index.access().file_prefix
    ));
    let mut out_file = BufWriter::new(
        OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(out_path)
            .await?,
    );
    out_file.write_all(&segment_index.header).await?;
    let mut in_file = BufReader::new(File::open(in_path).await?);
    match &segment_index.samples.inner {
        ArchivedSampleRunsInner::S32(runs) => {
            for run in runs.iter() {
                let (offset, len) = (run.0.to_native(), u64::from(run.1.to_native()));
                in_file.seek(SeekFrom::Start(offset.into())).await?;
                let n_written =
                    tokio::io::copy(&mut (&mut in_file).take(len), &mut out_file).await?;
                if n_written != len {
                    return Err(DashIndexError::SegmentWriteError(
                        format!(
                            "Unexpected EOF, failed to read {}B from {} at offset {}",
                            len,
                            in_path.display(),
                            offset
                        )
                        .into(),
                    ));
                }
            }
        }
        ArchivedSampleRunsInner::S64(runs) => {
            for run in runs.iter() {
                let (offset, len) = (run.0.to_native(), u64::from(run.1.to_native()));
                in_file.seek(SeekFrom::Start(offset)).await?;
                let n_written =
                    tokio::io::copy(&mut (&mut in_file).take(len), &mut out_file).await?;
                if n_written != len {
                    return Err(DashIndexError::SegmentWriteError(
                        format!(
                            "Unexpected EOF, failed to read {}B from {} at offset {}",
                            len,
                            in_path.display(),
                            offset
                        )
                        .into(),
                    ));
                }
            }
        }
    }
    out_file.flush().await?;
    Ok(())
}

fn make_index_segment(
    track: &Track,
    seg_idx: usize,
    ftyp: &Ftyp,
) -> Result<DiskSegmentIndex, DashIndexError> {
    let styp = Styp {
        major_brand: ftyp.major_brand,
        minor_version: ftyp.minor_version,
        compatible_brands: ftyp.compatible_brands.clone(),
    };

    let seg = track
        .segments
        .get(seg_idx)
        .ok_or(DashIndexError::SegmentIndexOutOfBounds)?;

    let default_sample_size: Option<u32> = seg
        .trun
        .iter()
        .map(|tr| tr.size)
        .all_equal_value()
        .unwrap_or_default();
    let default_sample_duration: Option<u32> = seg
        .trun
        .iter()
        .map(|tr| tr.duration)
        .all_equal_value()
        .unwrap_or_default();
    let default_sample_flags: Option<u32> = seg
        .trun
        .iter()
        .map(|tr| tr.flags)
        .all_equal_value()
        .unwrap_or_default();
    let default_cts_offset: Option<i64> = seg
        .trun
        .iter()
        .map(|tr| tr.cts)
        .all_equal_value()
        .unwrap_or_default();

    let traf = Traf {
        tfhd: Tfhd {
            track_id: track.track_id,
            default_base_is_moof: true,
            default_sample_duration,
            default_sample_size,
            default_sample_flags,
            ..Default::default()
        },
        tfdt: Some(Tfdt {
            base_media_decode_time: seg.start_dts,
        }),
        trun: Vec::from([Trun {
            data_offset: Some(1234),
            samples: TrunSamples::Explicit(
                seg.trun
                    .iter()
                    .map(|tr| TrunEntry {
                        size: default_sample_size.is_none().then_some(tr.size).flatten(),
                        cts: default_cts_offset.is_none().then_some(tr.cts).flatten(),
                        flags: default_sample_flags.is_none().then_some(tr.flags).flatten(),
                        duration: default_sample_duration
                            .is_none()
                            .then_some(tr.duration)
                            .flatten(),
                    })
                    .collect(),
            ),
        }]),
        ..Default::default()
    };
    let mut moof = Moof {
        mfhd: Mfhd {
            sequence_number: (seg_idx + 1).try_into().unwrap(),
        },
        traf: vec![traf],
    };
    let mdat_body_size: u64 = seg.samples.total_len()?;
    let mdat_size32 = u32::try_from(mdat_body_size.try_add(8u64)?);
    let mdat_header_len = if mdat_size32.is_ok() { 8 } else { 16 };

    let mut tmp: Vec<u8> = Vec::new();
    moof.encode(&mut tmp)?;
    moof.traf[0].trun[0].data_offset = Some(
        i32::try_from(tmp.len().try_add(mdat_header_len)?)
            .map_err(|_| DashIndexError::IntOverflow)?,
    );

    let mut header: Vec<u8> = Vec::new();
    styp.encode(&mut header)?;
    moof.encode(&mut header)?;
    if let Ok(size_32) = mdat_size32 {
        header.extend_from_slice(&size_32.to_be_bytes());
        header.extend_from_slice(b"mdat");
    } else {
        header.extend_from_slice(&1u32.to_be_bytes());
        header.extend_from_slice(b"mdat");
        header.extend_from_slice(&(mdat_body_size + 16).to_be_bytes());
    }
    Ok(DiskSegmentIndex {
        header,
        samples: seg.samples.clone(),
    })
}

async fn index_moov<P: AsRef<Path>>(
    ftyp: &Ftyp,
    moov: &Moov,
    out_dir: P,
    file_prefix: &str,
    mpd_name: &str,
    segment_duration: f32,
) -> Result<(), DashIndexError> {
    let out_dir = out_dir.as_ref();
    let mut video_track = None;
    let mut audio_track = None;
    let mut index_audio = Vec::new();
    let mut index_video = Vec::new();
    if moov.mvex.is_some() {
        return Err(DashIndexError::NotSupported);
    }
    for trak in &moov.trak {
        match trak.mdia.hdlr.handler.as_ref() {
            b"vide" => {
                let base = index_track(trak, segment_duration)?;
                let video = VideoTrack {
                    width: trak.tkhd.width.integer().into(),
                    height: trak.tkhd.height.integer().into(),
                    base,
                };
                let init_segment_moov = make_init_segment(moov, trak)?;
                let out_file = OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .create(true)
                    .open(&out_dir.join(format!("{file_prefix}-video-init.mp4")))
                    .await?;
                let mut out_file = BufWriter::new(out_file);
                ftyp.write_to(&mut out_file).await?;
                init_segment_moov.write_to(&mut out_file).await?;
                out_file.flush().await?;
                for seg_idx in 0..video.base.segments.len() {
                    index_video.push(make_index_segment(&video.base, seg_idx, ftyp)?);
                }
                video_track = Some(video);
            }
            b"soun" => {
                let base = index_track(trak, segment_duration)?;
                let audio = AudioTrack { base };
                let init_segment_moov = make_init_segment(moov, trak)?;
                let out_file = OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .create(true)
                    .open(out_dir.join(format!("{file_prefix}-audio-init.mp4")))
                    .await?;
                let mut out_file = BufWriter::new(out_file);
                ftyp.write_to(&mut out_file).await?;
                init_segment_moov.write_to(&mut out_file).await?;
                out_file.flush().await?;
                for seg_idx in 0..audio.base.segments.len() {
                    index_audio.push(make_index_segment(&audio.base, seg_idx, ftyp)?);
                }
                audio_track = Some(audio);
            }
            other => {
                let handler_name = String::from_utf8_lossy(other.as_slice());
                debug!(?handler_name, "skip unknown handler");
            }
        };
    }
    if video_track.is_none() && audio_track.is_none() {
        return Err(DashIndexError::ParseError(
            "No video or audio track found".into(),
        ));
    }
    let disk_index = DiskIndex {
        segments_video: index_video,
        segments_audio: index_audio,
        file_prefix: file_prefix.to_owned(),
    };
    let mut index_file = BufWriter::new(
        OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(out_dir.join(format!("{file_prefix}.dash_index")))
            .await?,
    );
    index_file.write_all(&1u64.to_le_bytes()).await?; // version
    index_file
        .write_all(
            &rkyv::to_bytes::<rkyv::rancor::Error>(&disk_index)
                .map_err(|err| DashIndexError::SerializationError(Box::new(err)))?,
        )
        .await?;
    index_file.flush().await?;

    let ad_video = video_track.as_ref().map(|video| {
        let tl_segments: Vec<mpd::S> = {
            let mut segs = Vec::default();
            for seg in video.base.segments.iter() {
                if segs.is_empty() {
                    segs.push(mpd::S {
                        t: Some(0),
                        d: seg.duration,
                        ..Default::default()
                    })
                } else if let Some(s) = segs.last_mut()
                    && s.d == seg.duration
                {
                    *s.r.get_or_insert_default() += 1;
                } else {
                    segs.push(mpd::S {
                        d: seg.duration,
                        ..Default::default()
                    })
                }
            }
            segs
        };
        mpd::AdaptationSet {
            segmentAlignment: Some(true),
            startWithSAP: Some(1),
            mimeType: Some("video/mp4".to_owned()),
            contentType: Some("video".to_owned()),
            maxWidth: Some(video.width.into()),
            maxHeight: Some(video.height.into()),
            SegmentTemplate: Some(mpd::SegmentTemplate {
                media: Some(format!("{file_prefix}-video-$Number$.m4s",)),
                initialization: Some(format!("{file_prefix}-video-init.mp4",)),
                startNumber: Some(0),
                timescale: Some(video.base.timebase.into()),
                SegmentTimeline: Some(mpd::SegmentTimeline {
                    segments: tl_segments,
                }),
                ..Default::default()
            }),
            representations: vec![mpd::Representation {
                id: Some(format!("{}", 0)),
                width: Some(video.width.into()),
                height: Some(video.height.into()),
                codecs: Some(video.base.codec.clone()),
                ..Default::default()
            }],
            ..Default::default()
        }
    });

    let ad_audio = audio_track.as_ref().map(|audio| {
        let tl_segments: Vec<mpd::S> = {
            let mut segs = Vec::default();
            for seg in audio.base.segments.iter() {
                if segs.is_empty() {
                    segs.push(mpd::S {
                        t: Some(0),
                        d: seg.duration,
                        ..Default::default()
                    })
                } else if let Some(s) = segs.last_mut()
                    && s.d == seg.duration
                {
                    *s.r.get_or_insert_default() += 1;
                } else {
                    segs.push(mpd::S {
                        d: seg.duration,
                        ..Default::default()
                    })
                }
            }
            segs
        };
        mpd::AdaptationSet {
            startWithSAP: Some(1),
            contentType: Some("audio".to_owned()),
            mimeType: Some("audio/mp4".to_owned()),
            SegmentTemplate: Some(mpd::SegmentTemplate {
                media: Some(format!("{file_prefix}-audio-$Number$.m4s",)),
                initialization: Some(format!("{file_prefix}-audio-init.mp4",)),
                startNumber: Some(0),
                timescale: Some(audio.base.timebase.into()),
                SegmentTimeline: Some(mpd::SegmentTimeline {
                    segments: tl_segments,
                }),
                ..Default::default()
            }),
            representations: vec![mpd::Representation {
                codecs: Some(audio.base.codec.clone()),
                ..Default::default()
            }],
            ..Default::default()
        }
    });
    let max_segment_duration_video = video_track.and_then(|v| {
        v.base
            .segments
            .iter()
            .map(|s| s.duration)
            .max()
            .map(|n| Duration::from_secs_f64(n as f64 / v.base.timebase as f64))
    });
    let max_segment_duration_audio = audio_track.and_then(|a| {
        a.base
            .segments
            .iter()
            .map(|s| s.duration)
            .max()
            .map(|n| Duration::from_secs_f64(n as f64 / a.base.timebase as f64))
    });
    let max_segment_duration = max_segment_duration_audio
        .into_iter()
        .chain(max_segment_duration_video)
        .max();

    let mpd_duration =
        Duration::from_secs_f64(moov.mvhd.duration as f64 / moov.mvhd.timescale as f64);
    let manifest = mpd::MPD {
        xmlns: Some("urn:mpeg:dash:schema:mpd:2011".to_owned()),
        mpdtype: Some("static".to_owned()),
        mediaPresentationDuration: Some(mpd_duration),
        profiles: Some("urn:mpeg:dash:profile:full:2011".to_owned()),
        periods: vec![mpd::Period {
            duration: Some(mpd_duration),
            adaptations: ad_video.into_iter().chain(ad_audio).collect(),
            ..Default::default()
        }],
        maxSegmentDuration: max_segment_duration,
        ..Default::default()
    };
    OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(out_dir.join(mpd_name))
        .await?
        .write_all(manifest.to_string().as_bytes())
        .await?;
    Ok(())
}

#[derive(Clone)]
pub struct DeserializedIndex(Vec<u8>);

impl DeserializedIndex {
    pub fn new(data: Vec<u8>) -> Result<Self, DashIndexError> {
        let archive = match data.as_slice().split_at_checked(8) {
            Some((version, rest)) if u64::from_le_bytes(version.try_into().unwrap()) == 1 => rest,
            _ => return Err(DashIndexError::CorruptIndex),
        };
        let _ = rkyv::access::<ArchivedDiskIndex, rkyv::rancor::Error>(archive)
            .map_err(|err| DashIndexError::DeserializationError(Box::new(err)))?;
        Ok(Self(data))
    }

    pub fn access(&self) -> &ArchivedDiskIndex {
        // SAFETY: archived was validated in Self::new
        unsafe { rkyv::access_unchecked::<ArchivedDiskIndex>(&self.0[4..]) }
    }
}

#[tracing::instrument(level = "debug")]
pub async fn index_file(
    in_path: &Path,
    out_dir: &Path,
    file_prefix: &str,
    mpd_name: &str,
    segment_duration: f32,
) -> Result<(), DashIndexError> {
    let mut reader = BufReader::new(File::open(in_path).await?);
    let mut ftyp = Ftyp::read_from(&mut reader).await?;
    ftyp.major_brand = FourCC::new(b"iso6");
    ftyp.minor_version = 1;
    ftyp.compatible_brands = vec![FourCC::new(b"iso6"), FourCC::new(b"dash")];

    let moov = loop {
        let header = Header::read_from(&mut reader).await?;
        if header.kind == Moov::KIND {
            break Moov::read_atom(&header, &mut reader).await?;
        } else {
            if let Some(size) = header.size {
                reader
                    .seek(SeekFrom::Current(size.try_into().unwrap()))
                    .await?;
            } else {
                // last box, extends to EOF
                return Err(DashIndexError::ParseError("No moov atom found".into()));
            }
        }
    };
    index_moov(
        &ftyp,
        &moov,
        out_dir,
        file_prefix,
        mpd_name,
        segment_duration,
    )
    .await?;
    Ok(())
}

trait UsizeExt: Sized + Copy {
    fn as_usize(self) -> usize;
}

impl UsizeExt for u32 {
    fn as_usize(self) -> usize {
        self.try_into().unwrap()
    }
}

impl UsizeExt for u64 {
    fn as_usize(self) -> usize {
        self.try_into().unwrap()
    }
}

trait IntExt: Sized + Copy {
    fn try_add(&self, other: Self) -> Result<Self, DashIndexError>;
}

macro_rules! impl_int_ext {
    ($typ:ident) => {
        impl IntExt for $typ {
            fn try_add(&self, other: Self) -> Result<Self, DashIndexError> {
                self.checked_add(other).ok_or(DashIndexError::IntOverflow)
            }
        }
    };
}

impl_int_ext!(u32);
impl_int_ext!(u64);
impl_int_ext!(i64);
impl_int_ext!(usize);
