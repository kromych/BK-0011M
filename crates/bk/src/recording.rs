//! Reading tape recordings: PCM WAV files and MP3s (many archived BK tapes
//! are MP3s; at 128 kbit/s and up the pulse edges come through unmoved).

use crate::audio::Wav;

/// A tape recording from a file's bytes: a PCM WAV file or an MP3. The
/// channels of a stereo recording are lined up and combined
/// ([`Wav::from_channels`]).
pub fn parse_recording(bytes: Vec<u8>) -> Result<Wav, String> {
    if bytes.starts_with(b"RIFF") {
        return Wav::parse(&bytes);
    }
    let (rate, channels) = decode_mp3(bytes)?;
    Ok(Wav::from_channels(rate, channels))
}

/// Decode an MP3 into its sample rate and channels. A file cut short ends
/// where it ends, and damaged frames are skipped, as players do.
fn decode_mp3(bytes: Vec<u8>) -> Result<(u32, Vec<Vec<f32>>), String> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::DecoderOptions;
    use symphonia::core::errors::Error;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let stream = MediaSourceStream::new(Box::new(std::io::Cursor::new(bytes)), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("mp3");
    let mut format = symphonia::default::get_probe()
        .format(
            &hint,
            stream,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|_| "not a WAV or MP3 file")?
        .format;
    let track = format.default_track().ok_or("no audio in the file")?;
    let (id, params) = (track.id, track.codec_params.clone());
    let rate = params.sample_rate.ok_or("the MP3 has no sample rate")?;
    let mut decoder = symphonia::default::get_codecs()
        .make(&params, &DecoderOptions::default())
        .map_err(|e| e.to_string())?;
    let mut channels: Vec<Vec<f32>> = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(Error::IoError(_)) => break,
            Err(e) => return Err(e.to_string()),
        };
        if packet.track_id() != id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(Error::DecodeError(_)) => continue,
            Err(e) => return Err(e.to_string()),
        };
        let spec = *decoded.spec();
        let n = spec.channels.count().max(1);
        let mut samples = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        samples.copy_interleaved_ref(decoded);
        channels.resize(channels.len().max(n), Vec::new());
        for frame in samples.samples().chunks_exact(n) {
            for (ch, &s) in channels.iter_mut().zip(frame) {
                ch.push(s);
            }
        }
    }
    if channels.iter().all(Vec::is_empty) {
        return Err("no audio in the MP3".into());
    }
    Ok((rate, channels))
}
