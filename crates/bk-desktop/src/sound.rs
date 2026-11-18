//! Live audio output through cpal. The emulator pushes samples; the audio
//! callback plays them, fading to silence if it runs dry.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

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
        if supported.sample_format() != cpal::SampleFormat::F32 {
            return Err(format!(
                "unsupported sample format {:?}",
                supported.sample_format()
            ));
        }
        let config = supported.config();
        let channels = usize::from(config.channels);
        let rate = config.sample_rate;
        let queue = Arc::new(Mutex::new(VecDeque::new()));
        let q = Arc::clone(&queue);
        let mut last = 0.0f32;
        let stream = device
            .build_output_stream::<f32, _, _>(
                config,
                move |out: &mut [f32], _| {
                    let mut q = q.lock().unwrap();
                    for frame in out.chunks_mut(channels) {
                        last = q.pop_front().unwrap_or(last * 0.99);
                        frame.fill(last);
                    }
                },
                |e| eprintln!("bk: audio: {e}"),
                None,
            )
            .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        Ok(Sound {
            _stream: stream,
            queue,
            rate,
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
