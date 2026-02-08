/// Simple all-pass filter for reverb
pub struct AllPass {
    buffer: Vec<f32>,
    pos: usize,
    feedback: f32,
}

impl AllPass {
    pub fn new(delay_samples: usize, feedback: f32) -> Self {
        Self {
            buffer: vec![0.0; delay_samples],
            pos: 0,
            feedback,
        }
    }

    pub fn process(&mut self, input: f32) -> f32 {
        let delayed = self.buffer[self.pos];
        let output = -input + delayed;
        self.buffer[self.pos] = input + delayed * self.feedback;
        self.pos = (self.pos + 1) % self.buffer.len();
        output
    }
}

/// Simple comb filter for reverb
pub struct Comb {
    buffer: Vec<f32>,
    pos: usize,
    feedback: f32,
    damping: f32,
    filter_state: f32,
}

impl Comb {
    pub fn new(delay_samples: usize, feedback: f32, damping: f32) -> Self {
        Self {
            buffer: vec![0.0; delay_samples],
            pos: 0,
            feedback,
            damping,
            filter_state: 0.0,
        }
    }

    pub fn process(&mut self, input: f32) -> f32 {
        let delayed = self.buffer[self.pos];

        // One-pole lowpass filter for damping
        self.filter_state = delayed * (1.0 - self.damping) + self.filter_state * self.damping;

        self.buffer[self.pos] = input + self.filter_state * self.feedback;
        self.pos = (self.pos + 1) % self.buffer.len();

        delayed
    }
}

/// Schroeder reverb with multiple combs and all-passes
pub struct Reverb {
    combs: Vec<Comb>,
    allpasses: Vec<AllPass>,
    wet: f32,
    dry: f32,
}

impl Reverb {
    pub fn new(sample_rate: f32, wet: f32, dry: f32) -> Self {
        // Prime number delays to avoid metallic resonances
        let comb_delays = [1557, 1617, 1491, 1422, 1277, 1356, 1188, 1116];
        let allpass_delays = [225, 556, 441, 341];

        let scale = sample_rate / 44100.0;

        let combs: Vec<Comb> = comb_delays
            .iter()
            .map(|&delay| {
                let scaled_delay = (delay as f32 * scale) as usize;
                Comb::new(scaled_delay, 0.84, 0.2)
            })
            .collect();

        let allpasses: Vec<AllPass> = allpass_delays
            .iter()
            .map(|&delay| {
                let scaled_delay = (delay as f32 * scale) as usize;
                AllPass::new(scaled_delay, 0.5)
            })
            .collect();

        Self {
            combs,
            allpasses,
            wet,
            dry,
        }
    }

    pub fn process(&mut self, input: f32) -> f32 {
        // Sum all comb filters
        let mut comb_sum = 0.0;
        for comb in &mut self.combs {
            comb_sum += comb.process(input);
        }
        comb_sum /= self.combs.len() as f32;

        // Chain all-pass filters
        let mut output = comb_sum;
        for allpass in &mut self.allpasses {
            output = allpass.process(output);
        }

        // Mix wet and dry
        self.dry * input + self.wet * output
    }

    pub fn set_mix(&mut self, wet: f32, dry: f32) {
        self.wet = wet;
        self.dry = dry;
    }
}

/// Simple one-pole lowpass filter
pub struct OnePole {
    state: f32,
    coeff: f32,
}

impl OnePole {
    pub fn new(sample_rate: f32, cutoff_hz: f32) -> Self {
        let coeff = Self::calculate_coeff(sample_rate, cutoff_hz);
        Self { state: 0.0, coeff }
    }

    fn calculate_coeff(sample_rate: f32, cutoff_hz: f32) -> f32 {
        let omega = 2.0 * std::f32::consts::PI * cutoff_hz / sample_rate;
        omega / (1.0 + omega)
    }

    pub fn process(&mut self, input: f32) -> f32 {
        self.state = self.state + self.coeff * (input - self.state);
        self.state
    }

    pub fn set_cutoff(&mut self, sample_rate: f32, cutoff_hz: f32) {
        self.coeff = Self::calculate_coeff(sample_rate, cutoff_hz);
    }
}

/// Effects chain for dreamy/melancholic processing
pub struct EffectsChain {
    reverb: Reverb,
    lowpass: OnePole,
    chorus_delay: Vec<f32>,
    chorus_pos: usize,
    chorus_phase: f32,
    sample_rate: f32,
}

impl EffectsChain {
    pub fn new_dreamy(sample_rate: f32) -> Self {
        let reverb = Reverb::new(sample_rate, 0.4, 0.6);
        let lowpass = OnePole::new(sample_rate, 4000.0); // Darker tone
        let chorus_delay = vec![0.0; 4410]; // ~100ms at 44.1kHz

        Self {
            reverb,
            lowpass,
            chorus_delay,
            chorus_pos: 0,
            chorus_phase: 0.0,
            sample_rate,
        }
    }

    pub fn process(&mut self, input: f32) -> f32 {
        // Lowpass for warmth and darkness (melancholic)
        let filtered = self.lowpass.process(input);

        // Simple chorus for width and shimmer (dreamy)
        let chorus = self.process_chorus(filtered);

        // Lush reverb for space (dreamy)
        let with_reverb = self.reverb.process(chorus);

        with_reverb
    }

    fn process_chorus(&mut self, input: f32) -> f32 {
        // LFO for chorus modulation
        let lfo_freq = 0.5; // Hz
        self.chorus_phase += 2.0 * std::f32::consts::PI * lfo_freq / self.sample_rate;
        if self.chorus_phase > 2.0 * std::f32::consts::PI {
            self.chorus_phase -= 2.0 * std::f32::consts::PI;
        }

        let lfo = self.chorus_phase.sin();
        let delay_offset = (lfo * 1000.0 + 2000.0) as usize; // 1-3ms modulation

        let delayed_pos =
            (self.chorus_pos + self.chorus_delay.len() - delay_offset) % self.chorus_delay.len();
        let delayed = self.chorus_delay[delayed_pos];

        self.chorus_delay[self.chorus_pos] = input;
        self.chorus_pos = (self.chorus_pos + 1) % self.chorus_delay.len();

        // Mix dry and chorus
        0.7 * input + 0.3 * delayed
    }
}
