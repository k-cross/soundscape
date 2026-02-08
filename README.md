# Soundscape - Reactive Procedural Audio Engine

A rust-based procedural audio thing that generates soundscapes reacting to microphone input and keyboard activity. It uses `tunes` and `fundsp`/`rundsp` for synthesis while `cpal` is used for mic input and `crossterm` is used for keyboard semantics. This is really janky, but kinda fun.

## Features

- procedural audio generation: Uses `fundsp` and `tunes` to create dsp graphs
- microphone input: ambient noise level influences the soundscape
- keyboard input: typing adds _energy_ and variability to the system
- modes:
    - `reactive`: a drone synthesizer that becomes brighter, more chaotic, and louder with input
    - `hybrid`: a pentatonic minor arpeggio that speeds up and has random octave jumps/note choices based on input _energy_
    - `dreamy`: an audio processor that takes microphone input and adds effects to it playing it back

## Usage

Run the project using `cargo`, uses `stable` rust:

```sh
cargo run --release -- -h
Usage: soundscape [OPTIONS]

Options:
  -m, --mode <MODE>      Operation mode [default: hybrid] [possible values: reactive, hybrid, dreamy]
      --list-devices     List available input devices and exit
      --device <DEVICE>  Name (or substring) of the input device to use
  -h, --help             Print help
  -V, --version          Print version
```

### Modes

- `reactive` (Default): Drone mode.
- `hybrid`: Arpeggiator mode.
- `dreamy`: Microphone augmentation mode.

### Options

- `--list-devices`: List available audio input devices.
- `--device <NAME>`: Specify an input device by name (or substring).

### Controls

- microphone: make noise to increase _energy_
- keyboard: type keys to add bursts of _energy_
    - ctrl+c: exit

## Granular Voice Processor - Dreamy & Melancholic

A real-time audio processor that transforms microphone input using granular synthesis to create dreamy, melancholic soundscapes.

### What It Does

This application captures your microphone input and processes it through:

1. Granular Synthesis Engine: breaks audio into _grains_ (50-200ms fragments) and reconstructs them with:
   - pitch shifting (slightly lower for melancholy)
   - time-stretching and randomization
   - multiple overlapping grains for rich texture

2. Effects Chain:
   - reverb: lush schroeder reverb for spaciousness
   - lowpass filter: warm, dark tone (cuts above 4khz)
   - chorus: gentle modulation for width and shimmer

_Warning_: You may hear feedback if your microphone picks up the speaker output. Use headphones or reduce speaker volume! Although I am currently experimenting with Acoustic Echo Cancellation to prevent this issue.

### Audio Flow

```
Microphone → Grain Buffer → Granular Engine → Effects Chain → Speakers
                ↓
           [Stores 2s of audio]
                ↓
         [20 grains/second]
         [120ms grain size]
         [0.92x pitch shift]
                ↓
           Reverb + Filter + Chorus
                ↓
           Dreamy Output!
```

### Tunable Parameters

#### `granular.rs` - `DreamyPreset::configure_engine()`

```rust
// Grain size (50-300ms)
engine.grain_size_ms = 120.0;  
// Larger = smoother, more pad-like
// Smaller = more granular, glitchy

// Grain density (grains per second, 5-50)
engine.grain_density = 15.0;
// Higher = denser texture, may sound "busier"
// Lower = sparser, more space between grains

// Pitch shift (0.5-2.0)
engine.pitch_shift = 0.92;
// <1.0 = lower/darker (melancholic)
// >1.0 = higher/brighter (happier)
// Try 1.15 for uplifting, 0.8 for very dark

// Pitch randomness (0.0-0.3)
engine.pitch_randomness = 0.12;
// Higher = more shimmer/detuning
// 0.0 = all grains at same pitch

// Time randomness (0.0-1.0)
engine.time_randomness = 0.6;
// Higher = more cloud-like, less defined
// Lower = more rhythmic, clearer
```

#### `effects.rs` - `EffectsChain::new_dreamy()`

```rust
// Reverb mix (wet, dry)
let reverb = Reverb::new(sample_rate, 0.4, 0.6);
// First number = reverb amount (0.0-1.0)
// Second number = dry signal (0.0-1.0)
// Try (0.6, 0.4) for more spacious
// Try (0.2, 0.8) for more intimate

// Lowpass cutoff frequency
let lowpass = OnePole::new(sample_rate, 4000.0);
// Higher = brighter (try 6000.0)
// Lower = darker (try 2500.0)

// Chorus mix in process_chorus()
0.7 * input + 0.3 * delayed
// Adjust the 0.3 value (0.0-0.5)
// Higher = more chorus effect
```

#### `main.rs` - `build_output_stream()`

```rust
// Buffer size (how far back grains can reach)
let mut engine = GranularEngine::new(
    sample_rate,
    2000.0,  // milliseconds (1000-5000)
    64,      // max simultaneous grains (32-128)
);
```

### Effect Presets

It would be awesome to add some knobs and sliders to adjust some of these settings, but this documents where they're currently at. Maybe adding a `ratatui` interface would be neat here:

#### Current: Dreamy & Melancholic
- Pitch: 0.92 (slightly down)
- Grain size: 120ms (smooth)
- Density: 15 grains/sec
- Heavy reverb, dark tone

#### Others:

_Ethereal Dream_:
```rust
engine.grain_size_ms = 180.0;
engine.grain_density = 10.0;
engine.pitch_shift = 1.05;
engine.pitch_randomness = 0.2;
reverb wet: 0.6
```

_Haunted/Dark_:
```rust
engine.grain_size_ms = 200.0;
engine.grain_density = 8.0;
engine.pitch_shift = 0.75;
engine.pitch_randomness = 0.05;
lowpass cutoff: 2000.0
```

_Shimmer Cloud_:
```rust
engine.grain_size_ms = 80.0;
engine.grain_density = 25.0;
engine.pitch_shift = 1.0;
engine.pitch_randomness = 0.25;
```

### How Granular Synthesis Works

Instead of processing audio directly, granular synthesis:

1. stores recent audio in a circular buffer (2 seconds)
2. spawns "grains" - short windowed samples from the buffer
3. each grain:
   - starts at a random point in the recent past (0-1 second ago)
   - has its own pitch (with randomness around the target pitch)
   - uses a hann window envelope (smooth fade in/out)
   - lasts 50-200ms
4. multiple grains overlap and sum together (typically 10-30 at once)
5. the result is a shimmering, cloud-like transformation of the input

### Technical Details

- circular buffer: ring buffer for efficient sample storage
- grain windowing: hann window for smooth envelopes
- pitch shifting: via sample rate manipulation (playback speed)
- reverb: schroeder reverb (8 comb filters + 4 all-pass filters)
- interpolation: linear interpolation for smooth grain reading

### Troubleshooting

#### Feedback/Squealing: 
- use headphones
- reduce speaker volume
- increase distance between mic and speakers

#### Latency: 
- granular synthesis adds inherent latency (grain size + buffer)
- reduce buffer size for less latency
- smaller grains = less latency

#### Distortion/Clipping:
- too many grains active at once
- reduce grain density
- the engine auto-normalizes by `sqrt(grain_count)`

#### Sounds Too Glitchy:
- increase grain size (try 150-200ms)
- reduce pitch randomness
- increase grain density

#### Not Dreamy Enough:
- increase reverb wet mix (0.5-0.7)
- increase pitch randomness (0.15-0.25)
- add more time randomness (0.7-0.9)

## Acoustic Echo Cancellation (AEC) Guide

When you use speakers instead of headphones, the microphone picks up the sound coming from the speakers creating a feedback loop:

```
Mic → Processing → Speakers → (sound travels through air) → Mic → ...
```

This causes:
- annoying feedback/howling
- doubled/echoed voice
- system instability

_Acoustic Echo Cancellation_ solves this by:
1. storing a copy of previous output signals (what's sent to the speakers named aka the _reference signal_)
2. detect how sound travels from `speaker → air → microphone`
3. predict what the microphone will hear from the speakers
4. subtract the prediction from the actual microphone input

### Understanding It

#### The Algorithm: Normalized Least Mean Squares (NLMS)

The echo canceller uses an _adaptive filter_ that learns the room's acoustic properties:

```
Microphone Input = Your Voice + Echo from Speakers + Noise

Echo Canceller:
1. $\text{Estimates Echo} = \text{FIR_Filter}(\text{Speaker Output})$
2. $\text{Clean Signal} = \text{Mic Input} - \text{Estimated Echo}$
3. Adapts filter using the error: $Filter += μ × Error × Reference$
```

#### Key Components

1. Adaptive Filter (FIR)
- 1024 coefficients (taps)
- models ~23ms of acoustic path @ 44.1khz
- covers typical room echoes and speaker-to-mic delays

2. NLMS Adaptation
- updates filter weights in real-time
- normalized by signal power (prevents instability with loud sounds)
- step size (μ = 0.5) controls learning speed

3. Voice Activity Detector (VAD)
- detects when speaker output is active
- only adapts filter when there's sound to learn from
- prevents filter divergence during silence

### Signal Flow

```
Input Stream (Microphone)
    ↓
[Channel] → Output Stream
              ↓
         [Echo Canceller] ← Previous Speaker Output (reference)
              ↓ (cleaned signal)
         [Granular Engine]
              ↓
         [Effects Chain]
              ↓
         Store as reference → (feeds back to Echo Canceller)
              ↓
         Output to Speakers
```

### Tunable Parameters

#### `echo_canceller.rs`

_Filter Length_ (default: 1024 samples)
```rust
let mut echo_canceller = EchoCanceller::new(1024, 0.5);
//                                          ^^^^
```
- smaller (256-512): faster adaptation, less cpu, only catches short echoes
- larger (2048-4096): catches longer echoes, more cpu, slower adaptation
- recommended: 1024 for typical rooms, 2048 for large spaces

_Step Size / Learning Rate_ (default: 0.5)
```rust
let mut echo_canceller = EchoCanceller::new(1024, 0.5);
//                                                ^^^
```
- smaller (0.1-0.3): slower learning, more stable, better steady-state
- larger (0.6-0.9): faster learning, less stable, tracks changes better
- recommended: 0.5 for general use, 0.7 if you move around a lot

_VAD Threshold_ (default: 0.0001)
```rust
let mut vad = VoiceActivityDetector::new(sample_rate, 0.0001);
//                                                     ^^^^^^
```
- lower: more sensitive, adapts more often, may adapt on noise
- higher: less sensitive, only adapts on clear sound
- recommended: 0.0001 for quiet environments, 0.001 for noisy ones

#### Advanced Tuning

Regularization Constant
```rust
regularization: 1e-6,  // Prevents division by zero
```
- increase (1e-4) if you get instability
- keep small for best performance

Attack/Release Times (VAD)
```rust
let attack_time = 0.010;   // 10ms
let release_time = 0.100;  // 100ms
```
- attack: how fast VAD responds to sound onset
- release: how long VAD stays active after sound stops
- faster attack = more responsive but might catch clicks
- longer release = smoother but might adapt during pauses

### Troubleshooting

_Note_: look into PID controllers to deal with some of these directly

#### Still Getting Feedback

Filter hasn't learned the acoustic path yet:
- let it run for 5-10 seconds to adapt
- increase step size to 0.7 for faster learning
- make sure VAD threshold isn't too high

#### Echo Cancellation Too Aggressive (Voice Sounds Thin)

Filter is removing too much:
- reduce step size to 0.3-0.4
- increase VAD threshold
- reduce filter length to 512

#### Weird Artifacts or Warbling

Filter is unstable or adapting incorrectly:
- reduce step size to 0.3
- increase VAD threshold to prevent adaptation on noise
- check speaker volume isn't too high

#### Doesn't Work in Large Rooms

Echoes arrive after filter length (>23ms)
- increase filter length to 2048 or 4096
- note: this increases cpu usage and memory

#### Works Initially, Then Gets Worse

Filter diverging (learning wrong things):
- VAD threshold might be too low (adapting on noise)
- increase VAD threshold
- reduce step size for more stability

### Performance Considerations

#### CPU Usage

Filter length directly impacts CPU:
- 512 taps: ~0.5% CPU (modern CPU)
- 1024 taps: ~1% CPU
- 2048 taps: ~2% CPU  
- 4096 taps: ~4% CPU

Formula: Approximately $\text{filter_length} \times \text{sample_rate} \times 2$ multiply-adds per second

#### Memory Usage

- filter weights: `filter_length × 4 bytes`
- reference buffer: `filter_length × 4 bytes`
- total for 1024: ~8kb (negligible)

#### Latency

Echo cancellation adds minimal latency:
- processing: <0.1ms
- no additional buffering needed
- does not increase overall system latency

### How the Algorithm Learns

The echo canceller is _adaptive_ - it continuously learns about the room:

Initialization (0-1 seconds):
- filter starts with all zeros
- first speaker outputs have no echo cancellation
- might hear initial feedback

Learning Phase (1-10 seconds):
- filter rapidly learns the dominant echo paths
- echo reduction improves quickly
- you'll noticably hear feedback decrease

Steady State (10+ seconds):
- filter learns most echo paths
- continues to fine-tune
- tracks slow changes (you moving, etc.)

Adaptation:
- only updates when speaker output is active (via VAD)
- uses NLMS to normalize learning based on volume
- self-corrects if you move or room changes

### Testing Echo Cancellation

Simple Test:
1. run the program with speakers (not headphones)
2. start with LOW speaker volume
3. speak - you should hear your processed voice
4. gradually increase speaker volume
5. compare to old version (without AEC) - should handle much higher volume

Stress Test:
1. set speaker volume moderately high
2. clap or make sudden sounds
3. echo canceller should adapt and remove speaker output
4. try moving around - it should re-adapt

Quality Test:
1. whisper quietly
2. echo canceller shouldn't remove your voice
3. only speaker playback should be removed

#### Automated Testing

Need to think of a way to get creative with inputs, maybe feeding samples as inputs then recording the outputs and feeding it back through a filter then measuring the overall output.

### Implemented AEC Features

This implementation is a simplification of professional AEC systems:
- [x] Adaptive filtering (NLMS)
- [x] Voice activity detection
- [x] Normalized learning rate
- [x] Real-time processing
- [ ] Non-linear processing (for speaker distortion)
- [ ] Double-talk detection (simultaneous near/far-end speech)
- [ ] Frequency-domain processing (more efficient for long filters)
- [ ] Noise suppression integration
- [ ] Comfort noise generation

For music/voice effects (our use case), this implementation is _sufficient_. For critical applications (VoIP, video conferencing), consider:
- WebRTC's AEC implementation
- Speex echo cancellation
- Commercial SDKs (Dolby, Krisp, etc.)

### Further Reading

- "Adaptive Filter Theory" by Simon Haykin
- "Digital Signal Processing" by Proakis & Manolakis  
- WebRTC AEC source code: https://webrtc.googlesource.com/src/+/main/modules/audio_processing/aec3/
- [LMS and NLMS Algorithms](https://en.wikipedia.org/wiki/Least_mean_squares_filter)

### Quick Reference: Common Adjustments

| Issue | Parameter | Direction |
|-------|-----------|-----------|
| Feedback persists | Step size | Increase to 0.7 |
| Voice sounds thin | Step size | Decrease to 0.3 |
| Large room echoes | Filter length | Increase to 2048 |
| CPU too high | Filter length | Decrease to 512 |
| Adapts on noise | VAD threshold | Increase to 0.001 |
| Won't adapt | VAD threshold | Decrease to 0.00001 |
