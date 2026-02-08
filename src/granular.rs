use rand::{Rng, SeedableRng};
use std::f32::consts::PI;

/// Circular buffer for storing audio samples
pub struct GrainBuffer {
    buffer: Vec<f32>,
    write_pos: usize,
    capacity: usize,
}

impl GrainBuffer {
    pub fn new(capacity_samples: usize) -> Self {
        Self {
            buffer: vec![0.0; capacity_samples],
            write_pos: 0,
            capacity: capacity_samples,
        }
    }

    pub fn write(&mut self, sample: f32) {
        self.buffer[self.write_pos] = sample;
        self.write_pos = (self.write_pos + 1) % self.capacity;
    }

    pub fn read(&self, position: f32) -> f32 {
        let pos = position.rem_euclid(self.capacity as f32);
        let idx = pos as usize;
        let frac = pos - idx as f32;

        let sample1 = self.buffer[idx];
        let sample2 = self.buffer[(idx + 1) % self.capacity];

        // Linear interpolation
        sample1 + (sample2 - sample1) * frac
    }
}

/// Individual grain parameters
pub struct Grain {
    pub start_pos: f32,
    pub current_pos: f32,
    pub length: usize,
    pub pitch: f32,
    pub amplitude: f32,
    pub pan: f32,
    pub active: bool,
}

impl Grain {
    pub fn new(start_pos: f32, length: usize, pitch: f32) -> Self {
        Self {
            start_pos,
            current_pos: 0.0,
            length,
            pitch,
            amplitude: 1.0,
            pan: 0.5,
            active: true,
        }
    }

    /// Hann window for smooth grain envelope
    pub fn window(&self) -> f32 {
        if !self.active {
            return 0.0;
        }
        let phase = self.current_pos / self.length as f32;
        if phase >= 1.0 {
            return 0.0;
        }
        0.5 * (1.0 - (2.0 * PI * phase).cos())
    }

    pub fn process(&mut self, buffer: &GrainBuffer) -> f32 {
        if !self.active {
            return 0.0;
        }

        let window = self.window();
        let read_pos = self.start_pos + self.current_pos * self.pitch;
        let sample = buffer.read(read_pos) * window * self.amplitude;

        self.current_pos += 1.0;
        if self.current_pos >= self.length as f32 {
            self.active = false;
        }

        sample
    }
}

/// Main granular synthesis engine
pub struct GranularEngine {
    buffer: GrainBuffer,
    grains: Vec<Grain>,
    max_grains: usize,
    sample_rate: f32,

    // Granular parameters
    pub grain_size_ms: f32,
    pub grain_density: f32, // grains per second
    pub pitch_shift: f32,
    pub pitch_randomness: f32,
    pub time_randomness: f32,

    // Internal state
    time_until_next_grain: f32,
    rng: rand::rngs::StdRng,
}

impl GranularEngine {
    pub fn new(sample_rate: f32, buffer_size_ms: f32, max_grains: usize) -> Self {
        let buffer_samples = (buffer_size_ms * sample_rate / 1000.0) as usize;

        Self {
            buffer: GrainBuffer::new(buffer_samples),
            grains: Vec::with_capacity(max_grains),
            max_grains,
            sample_rate,
            grain_size_ms: 100.0,
            grain_density: 20.0,
            pitch_shift: 1.0,
            pitch_randomness: 0.05,
            time_randomness: 0.3,
            time_until_next_grain: 0.0,
            rng: rand::rngs::StdRng::from_rng(&mut rand::rng()),
        }
    }

    pub fn write_input(&mut self, sample: f32) {
        self.buffer.write(sample);
    }

    pub fn process(&mut self) -> f32 {
        // Update grain spawning timer
        self.time_until_next_grain -= 1.0;

        // Spawn new grain if needed
        if self.time_until_next_grain <= 0.0 && self.grains.len() < self.max_grains {
            self.spawn_grain();
            let interval = self.sample_rate / self.grain_density;
            let randomness = interval * self.time_randomness;
            self.time_until_next_grain = interval + self.rng.random_range(-randomness..randomness);
        }

        // Process all active grains and sum output
        let mut output = 0.0;
        let mut active_count = 0;

        for grain in &mut self.grains {
            if grain.active {
                output += grain.process(&self.buffer);
                active_count += 1;
            }
        }

        // Remove inactive grains
        self.grains.retain(|g| g.active);

        // Normalize by number of active grains to prevent clipping
        if active_count > 0 {
            output / (active_count as f32).sqrt()
        } else {
            output
        }
    }

    fn spawn_grain(&mut self) {
        let grain_size_samples = (self.grain_size_ms * self.sample_rate / 1000.0) as usize;

        // Random position in buffer (looking back in time)
        let lookback = self.rng.random_range(0.0..0.5);
        let start_pos = self.buffer.write_pos as f32 - (lookback * self.buffer.capacity as f32);

        // Pitch with randomness
        let pitch_variation = self
            .rng
            .random_range(-self.pitch_randomness..self.pitch_randomness);
        let pitch = self.pitch_shift * (1.0 + pitch_variation);

        let grain = Grain::new(start_pos, grain_size_samples, pitch);
        self.grains.push(grain);
    }
}

/// Dreamy/melancholic preset parameters
pub struct DreamyPreset;

impl DreamyPreset {
    pub fn configure_engine(engine: &mut GranularEngine) {
        // Larger grains for smoother texture
        engine.grain_size_ms = 120.0;

        // Medium density for ethereal quality
        engine.grain_density = 15.0;

        // Slight pitch down for melancholy
        engine.pitch_shift = 0.92;

        // More pitch variation for dreamy shimmer
        engine.pitch_randomness = 0.12;

        // High time randomness for cloudy texture
        engine.time_randomness = 0.6;
    }
}
