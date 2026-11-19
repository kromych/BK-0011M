//! Live audio output through cpal. The emulator pushes samples; the audio
//! callback plays them, fading to silence if it runs dry.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};

pub struct Sound {
    _stream: cpal::Stream,
    queue: Arc<Mutex<VecDeque<f32>>>,
    pub rate: u32,
}

impl Sound {
    pub fn open() -> Result<Sound, String> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or("no audio output device")?;
        let supported = device.default_output_config().map_err(|e| e.to_string())?;
        let config = supported.config();
        let queue = Arc::new(Mutex::new(VecDeque::new()));
        // Whatever sample format the device takes.
        let stream = match supported.sample_format() {
            SampleFormat::F32 => build::<f32>(&device, &config, &queue),
            SampleFormat::F64 => build::<f64>(&device, &config, &queue),
            SampleFormat::I16 => build::<i16>(&device, &config, &queue),
            SampleFormat::U16 => build::<u16>(&device, &config, &queue),
            SampleFormat::I32 => build::<i32>(&device, &config, &queue),
            SampleFormat::U32 => build::<u32>(&device, &config, &queue),
            SampleFormat::I8 => build::<i8>(&device, &config, &queue),
            SampleFormat::U8 => build::<u8>(&device, &config, &queue),
            other => Err(format!("unsupported sample format {other:?}")),
        }?;
        stream.play().map_err(|e| e.to_string())?;
        Ok(Sound {
            _stream: stream,
            queue,
            rate: config.sample_rate,
        })
    }

    /// Samples waiting to be played.
    pub fn queued(&self) -> usize {
        self.queue.lock().unwrap().len()
    }

    pub fn push(&self, samples: &[f32]) {
        self.queue.lock().unwrap().extend(samples);
    }
}

/// An output stream of samples of type `T`, each frame (all channels) one
/// sample from the queue.
fn build<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    queue: &Arc<Mutex<VecDeque<f32>>>,
) -> Result<cpal::Stream, String> {
    let channels = usize::from(config.channels);
    let q = Arc::clone(queue);
    let mut last = 0.0f32;
    device
        .build_output_stream::<T, _, _>(
            *config,
            move |out: &mut [T], _| {
                let mut q = q.lock().unwrap();
                for frame in out.chunks_mut(channels) {
                    last = q.pop_front().unwrap_or(last * 0.99);
                    frame.fill(T::from_sample(last));
                }
            },
            |e| eprintln!("bk: audio: {e}"),
            None,
        )
        .map_err(|e| e.to_string())
}
