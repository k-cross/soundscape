/// Adaptive Echo Canceller using Normalized Least Mean Squares (NLMS) algorithm
///
/// This removes acoustic feedback by:
/// 1. Keeping a reference of what's played through speakers
/// 2. Modeling the acoustic path from speaker to microphone
/// 3. Subtracting the estimated echo from the microphone signal
pub struct EchoCanceller {
    /// Adaptive filter coefficients (models speaker-to-mic path)
    filter_weights: Vec<f32>,

    /// Reference signal buffer (what's being played on speakers)
    reference_buffer: Vec<f32>,

    /// Current position in reference buffer
    buffer_pos: usize,

    /// Learning rate (step size) for adaptation
    step_size: f32,

    /// Regularization constant to prevent division by zero
    regularization: f32,

    /// Enable/disable adaptation (turn off when there's no far-end signal)
    adaptation_enabled: bool,
}

impl EchoCanceller {
    /// Create a new echo canceller
    ///
    /// * `filter_length` - number of taps in adaptive filter (typically 512-2048)
    ///                     longer = can model longer echo paths but slower adaptation
    /// * `step_size` - learning rate (0.1-0.9, typical: 0.5)
    ///                 higher = faster adaptation but less stable
    pub fn new(filter_length: usize, step_size: f32) -> Self {
        Self {
            filter_weights: vec![0.0; filter_length],
            reference_buffer: vec![0.0; filter_length],
            buffer_pos: 0,
            step_size,
            regularization: 1e-6,
            adaptation_enabled: true,
        }
    }

    /// Process one sample through the echo canceller
    pub fn process(&mut self, mic_input: f32, speaker_output: f32) -> f32 {
        // Store reference signal (what's playing on speakers)
        self.reference_buffer[self.buffer_pos] = speaker_output;

        // Estimate echo using adaptive filter
        let echo_estimate = self.compute_echo_estimate();

        // Subtract estimated echo from microphone input
        let error_signal = mic_input - echo_estimate;

        // Adapt filter weights using NLMS algorithm
        if self.adaptation_enabled {
            self.update_filter_weights(error_signal);
        }

        // Move to next buffer position
        self.buffer_pos = (self.buffer_pos + 1) % self.reference_buffer.len();

        error_signal
    }

    // Compute echo estimate by convolving reference signal with filter weights
    fn compute_echo_estimate(&self) -> f32 {
        let mut echo = 0.0;
        let filter_len = self.filter_weights.len();

        for i in 0..filter_len {
            // Read from buffer in reverse chronological order
            let buffer_idx = (self.buffer_pos + filter_len - i) % filter_len;
            echo += self.filter_weights[i] * self.reference_buffer[buffer_idx];
        }

        echo
    }

    // Update filter weights using Normalized Least Mean Squares (NLMS)
    fn update_filter_weights(&mut self, error: f32) {
        // Calculate reference signal power for normalization
        let mut power = 0.0;
        let filter_len = self.filter_weights.len();

        for i in 0..filter_len {
            let buffer_idx = (self.buffer_pos + filter_len - i) % filter_len;
            let sample = self.reference_buffer[buffer_idx];
            power += sample * sample;
        }

        // Normalized step size (prevents instability when reference is loud)
        let normalized_step = self.step_size / (power + self.regularization);

        // Update each filter coefficient
        for i in 0..filter_len {
            let buffer_idx = (self.buffer_pos + filter_len - i) % filter_len;
            let reference_sample = self.reference_buffer[buffer_idx];

            // NLMS weight update: w[i] += µ * error * x[i] / (||x||² + ε)
            self.filter_weights[i] += normalized_step * error * reference_sample;
        }
    }

    /// Reset the echo canceller (clears learned model)
    pub fn reset(&mut self) {
        for weight in &mut self.filter_weights {
            *weight = 0.0;
        }
        for sample in &mut self.reference_buffer {
            *sample = 0.0;
        }
    }

    /// Enable or disable filter adaptation
    pub fn set_adaptation_enabled(&mut self, enabled: bool) {
        self.adaptation_enabled = enabled;
    }

    /// Adjust step size (learning rate)
    pub fn set_step_size(&mut self, step_size: f32) {
        self.step_size = step_size.clamp(0.0, 1.0);
    }
}

/// Voice Activity Detector (VAD)
///
/// Detects when there's actual voice vs silence, which helps the echo canceller
/// know when to adapt (should only adapt when there's speaker output)
pub struct VoiceActivityDetector {
    /// Energy threshold for voice detection
    threshold: f32,

    /// Smoothed energy estimate
    energy: f32,

    /// Attack/release time constants
    attack: f32,
    release: f32,
}

impl VoiceActivityDetector {
    pub fn new(sample_rate: f32, threshold: f32) -> Self {
        // Time constants for smoothing (10ms attack, 100ms release)
        let attack_time = 0.010;
        let release_time = 0.100;

        Self {
            threshold,
            energy: 0.0,
            attack: (-1.0 / (sample_rate * attack_time)).exp(),
            release: (-1.0 / (sample_rate * release_time)).exp(),
        }
    }

    /// Process a sample and return whether voice is detected
    pub fn process(&mut self, sample: f32) -> bool {
        let instant_energy = sample * sample;

        // Smooth energy with attack/release
        if instant_energy > self.energy {
            self.energy = self.attack * self.energy + (1.0 - self.attack) * instant_energy;
        } else {
            self.energy = self.release * self.energy + (1.0 - self.release) * instant_energy;
        }

        self.energy > self.threshold
    }

    pub fn set_threshold(&mut self, threshold: f32) {
        self.threshold = threshold;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_echo_canceller_basic() {
        let mut aec = EchoCanceller::new(128, 0.5);

        // Simple impulse test
        let mic_input = 1.0;
        let speaker_output = 0.0;
        let result = aec.process(mic_input, speaker_output);

        // Should pass through when no echo
        assert!((result - mic_input).abs() < 0.01);
    }

    #[test]
    fn test_vad_silence() {
        let mut vad = VoiceActivityDetector::new(44100.0, 0.001);

        // Silent samples should not trigger VAD
        for _ in 0..100 {
            assert!(!vad.process(0.0));
        }
    }

    #[test]
    fn test_vad_active() {
        let mut vad = VoiceActivityDetector::new(44100.0, 0.001);

        // Loud samples should trigger VAD
        for _ in 0..100 {
            vad.process(0.5);
        }
        assert!(vad.process(0.5));
    }
}
