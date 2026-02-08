mod echo_canceller;
mod effects;
mod granular;

use anyhow::Result;
use clap::{Parser, ValueEnum};
use cpal::{
    FromSample, Host, Sample, SizedSample,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use crossbeam_channel::{Receiver, Sender, bounded};
use echo_canceller::{EchoCanceller, VoiceActivityDetector};
use effects::EffectsChain;
use fundsp::prelude32::*;
use granular::{DreamyPreset, GranularEngine};
use log::{debug, error};
use std::io::Write;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Operation mode
    #[arg(short, long, value_enum, default_value_t = Mode::Hybrid)]
    mode: Mode,

    /// List available input devices and exit
    #[arg(long)]
    list_devices: bool,

    /// Name (or substring) of the input device to use
    #[arg(long)]
    device: Option<String>,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
enum Mode {
    Reactive,
    Hybrid,
    Dreamy,
}

fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();

    // List devices if requested
    if args.list_devices {
        let host = cpal::default_host();
        println!("Available Input Devices:");
        if let Ok(devices) = host.input_devices() {
            for device in devices {
                if let Ok(name) = device.description() {
                    println!("  - {}", name);
                }
            }
        }
        return Ok(());
    }

    match args.mode {
        Mode::Reactive => reactive(args.device),
        Mode::Hybrid => hybrid(args.device),
        Mode::Dreamy => dreamy(args.device),
    }
}

fn reactive(device_name: Option<String>) -> Result<()> {
    // raw mode makes key presses immediate
    crossterm::terminal::enable_raw_mode()?;

    // Setup audio host
    let host = cpal::default_host();
    let device = host.default_output_device().expect("No output device");
    let config = device.default_output_config()?;

    let mic_level = shared(0.0f32);
    let mic_level_for_input = mic_level.clone();
    let mic_level_for_monitor = mic_level.clone();

    let key_level = keyboard_input(0.2);
    let key_level_for_monitor = key_level.clone();

    // The synthesis graph
    // A drone that gets "brighter" and more chaotic as the room gets louder
    let base_freq = 55.0f32; // Low A
    let mic = var(&mic_level);
    let key = var(&key_level);
    let control = mic + key;

    // Modulate frequency based on combined level (FM)
    let pitch_mod = control.clone() * 100.0;
    let freq1 = constant(base_freq) + pitch_mod.clone();
    let freq2 = constant(base_freq * 1.01) + pitch_mod;

    // Modulate filter cutoff
    let cutoff = constant(100.0) + (control.clone() * 4000.0);
    let q = constant(0.5);

    // Modulate volume (AM)
    let volume = constant(0.2) + (control.clone() * 2.0);

    // Lowpass filter takes (input, cutoff_frequency, Q)
    // combine oscs, then pipe into filter alongside cutoff and Q
    let oscs = (freq1 >> saw()) + (freq2 >> saw());
    let filtered = (oscs | cutoff | q) >> lowpass();

    // Apply volume
    let synth = filtered * volume;

    // Add texture separately
    let mut synth = synth + (pink() * 0.05);

    synth.reset();
    synth.set_sample_rate(config.sample_rate() as f64);

    // The input stream
    let input_device = get_mic(device_name, host)?;
    let input_config = input_device.default_input_config()?;
    let input_stream = input_device.build_input_stream(
        &input_config.into(),
        move |data: &[f32], _| {
            let rms = (data.iter().map(|&x| x * x).sum::<f32>() / data.len() as f32).sqrt();
            // Boost sensitivity by 5x
            mic_level_for_input.set_value(rms * 5.0);
        },
        |err| error!("Mic error: {}", err),
        None,
    )?;

    // The output stream
    let output_stream = device.build_output_stream(
        &config.into(),
        move |data: &mut [f32], _| {
            for frame in data.chunks_mut(2) {
                // Tick the graph.
                // We use explicit Frame type for 0 inputs (source).
                let vals = synth.tick(&Frame::default());
                let sample = vals[0];

                // Write to output buffer (stereo)
                frame[0] = sample;
                frame[1] = sample;
            }
        },
        |err| error!("Output error: {}", err),
        None,
    )?;

    input_stream.play()?;
    output_stream.play()?;

    println!("Reactive Procedural Engine Running...");
    println!("- Make noise OR type keys to influence sound.");

    // Monitor loop
    loop {
        std::thread::sleep(std::time::Duration::from_millis(50));
        let m = mic_level_for_monitor.value();
        let k = key_level_for_monitor.value();
        print!("Mic: {:.3} | Key: {:.3} | Total: {:.3}\r", m, k, m + k);
        std::io::stdout().flush().ok();
    }
}

fn hybrid(device_name: Option<String>) -> Result<()> {
    // raw mode makes key presses immediate
    crossterm::terminal::enable_raw_mode()?;

    // SHARED STATE: bridges between threads
    let mic_energy = shared(0.0f32); // Mic volume (0.0 to 1.0)
    let current_freq = shared(440.0f32); // The frequency decided by composer (base note)

    let energy_in = mic_energy.clone();
    let energy_logic = mic_energy.clone();
    let freq_logic = current_freq.clone();

    // AUDIO GRAPH
    // sine() (oscillator) driven by the variable frequency.
    let mut synth = var(&current_freq) >> sine() >> lowpass_hz(1000.0f32, 1.0f32) * 0.2f32; // Simple volume control, no envelope for now
    let host = cpal::default_host();
    let out_device = host.default_output_device().expect("No output");
    let config = out_device.default_output_config()?;

    // Config synthesis
    synth.reset();
    synth.set_sample_rate(config.sample_rate() as f64);

    // MICROPHONE INPUT
    let mic_device = get_mic(device_name, host)?;
    let in_stream = mic_device.build_input_stream(
        &mic_device.default_input_config()?.into(),
        move |data: &[f32], _| {
            let rms = (data.iter().map(|&x| x * x).sum::<f32>() / data.len() as f32).sqrt();
            energy_in.set_value(rms);
        },
        |err| error!("Input error: {}", err),
        None,
    )?;

    // KEYBOARD INPUT
    let key_energy = keyboard_input(0.025);
    let key_logic = key_energy.clone();

    // THE COMPOSER
    std::thread::spawn(move || {
        // Pentatonic Minor intervals (semitones from root)
        let intervals = [0, 3, 5, 7, 10, 12, 14, 17, 19, 21, 24];
        let root_hz = 130.81; // C3

        // Base Arpeggio Pattern (indices into intervals)
        let pattern = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1];
        let mut step = 0;

        loop {
            let mic_e = energy_logic.value();
            let key_e = key_logic.value();

            // Combined Energy
            // Mic provides a baseline, Keyboard provides sharp bursts
            let total_energy = mic_e + key_e; // Assuming key_e is 0.0-1.0

            // Variability Factors
            let chaos = total_energy * 10.0; // Amplified for effect
            let speed_mod = if total_energy > 0.05 {
                // Linearly interpolate speed from 250ms down to 50ms based on energy
                // quiet (0.05) -> 250ms
                // loud (0.5+) -> 50ms
                let factor = ((total_energy - 0.05) * 2.0).clamp(0.0, 1.0);
                (250.0 * (1.0 - factor) + 50.0 * factor) as u64
            } else {
                250
            };

            // Selection Logic
            let note_index = if rand::random::<f32>() < chaos {
                // Chaos! Pick a random note
                rand::random_range(0..intervals.len())
            } else {
                // Structure! Follow the pattern
                pattern[step % pattern.len()]
            };

            // Octave Logic (ocassionally with high chaos)
            let octave = if rand::random::<f32>() < chaos * 0.5 {
                1
            } else {
                0
            };

            // Calculate frequency
            debug!("Note index: {}", note_index);
            let semitones = intervals[note_index] + (octave * 12);
            let freq = root_hz * (2.0f32.powf(semitones as f32 / 12.0));

            freq_logic.set_value(freq);

            // Log for debugging/visualization
            if step % 8 == 0 {
                debug!(
                    "> Loop. Mic: {:.3}, Key: {:.3} -> Chaos: {:.2}",
                    mic_e, key_e, chaos
                );
            }

            // Advance step
            step += 1;

            std::thread::sleep(std::time::Duration::from_millis(speed_mod));
        }
    });

    // START OUTPUT
    let out_stream = out_device.build_output_stream(
        &config.into(),
        move |data: &mut [f32], _| {
            for frame in data.chunks_mut(2) {
                // Tick the graph.
                let vals = synth.tick(&Frame::default());
                let sample = vals[0];

                frame[0] = sample;
                frame[1] = sample;
            }
        },
        |err| error!("Output error: {}", err),
        None,
    )?;

    in_stream.play()?;
    out_stream.play()?;
    println!("Hybrid Procedural Engine Running...");
    println!("- Make noise to influence the sound.");
    println!("- Type on keyboard to add variability.");
    std::thread::park();

    crossterm::terminal::disable_raw_mode()?;
    Ok(())
}

fn keyboard_input(sensitivity: f32) -> Shared {
    let energy = shared(0.0f32);
    let energy_out = energy.clone();

    std::thread::spawn(move || {
        let mut current_energy = 0.0f32;
        loop {
            // Check for event (non-blocking if timeout is 0, but we want some sleep)
            if crossterm::event::poll(std::time::Duration::from_millis(50)).unwrap_or(false) {
                if let Ok(crossterm::event::Event::Key(key_event)) = crossterm::event::read() {
                    // Handle Ctrl+C to exit
                    if key_event.code == crossterm::event::KeyCode::Char('c')
                        && key_event
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL)
                    {
                        let _ = crossterm::terminal::disable_raw_mode();
                        println!("\r\nExiting...");
                        std::process::exit(0);
                    }

                    // Impulsively add energy
                    current_energy += sensitivity;
                }
            }

            current_energy *= 0.95; // decay
            current_energy = current_energy.clamp(0.0, 1.0);
            energy.set_value(current_energy);
        }
    });

    energy_out
}

fn dreamy(device_name: Option<String>) -> Result<()> {
    // Setup audio host
    let host = cpal::default_host();
    let output_device = host
        .default_output_device()
        .expect("No output device available");

    println!("Output device: {}", output_device.description()?);

    // Get input config
    let input_device = get_mic(device_name, host)?;
    let input_config = input_device.default_input_config()?;
    println!("Input config: {:?}", input_config);

    // Get output config
    let output_config = output_device.default_output_config()?;
    println!("Output config: {:?}\n", output_config);

    let sample_rate = input_config.sample_rate();

    // Create channel for passing audio between input and output streams
    let (tx, rx): (Sender<f32>, Receiver<f32>) = bounded(8192);

    // Build input stream
    let input_stream = match input_config.sample_format() {
        cpal::SampleFormat::I8 => {
            build_input_stream::<i8>(&input_device, &input_config.into(), tx)?
        }
        cpal::SampleFormat::I16 => {
            build_input_stream::<i16>(&input_device, &input_config.into(), tx)?
        }
        cpal::SampleFormat::I32 => {
            build_input_stream::<i32>(&input_device, &input_config.into(), tx)?
        }
        cpal::SampleFormat::I64 => {
            build_input_stream::<i64>(&input_device, &input_config.into(), tx)?
        }
        cpal::SampleFormat::U8 => {
            build_input_stream::<u8>(&input_device, &input_config.into(), tx)?
        }
        cpal::SampleFormat::U16 => {
            build_input_stream::<u16>(&input_device, &input_config.into(), tx)?
        }
        cpal::SampleFormat::U32 => {
            build_input_stream::<u32>(&input_device, &input_config.into(), tx)?
        }
        cpal::SampleFormat::U64 => {
            build_input_stream::<u64>(&input_device, &input_config.into(), tx)?
        }
        cpal::SampleFormat::F32 => {
            build_input_stream::<f32>(&input_device, &input_config.into(), tx)?
        }
        cpal::SampleFormat::F64 => {
            build_input_stream::<f64>(&input_device, &input_config.into(), tx)?
        }
        sample_format => panic!("Unsupported sample format: {}", sample_format),
    };

    // Build output stream
    let output_stream = match output_config.sample_format() {
        cpal::SampleFormat::I8 => build_output_stream::<i8>(
            &output_device,
            &output_config.into(),
            rx,
            sample_rate as f32,
        )?,
        cpal::SampleFormat::I16 => build_output_stream::<i16>(
            &output_device,
            &output_config.into(),
            rx,
            sample_rate as f32,
        )?,
        cpal::SampleFormat::I32 => build_output_stream::<i32>(
            &output_device,
            &output_config.into(),
            rx,
            sample_rate as f32,
        )?,
        cpal::SampleFormat::I64 => build_output_stream::<i64>(
            &output_device,
            &output_config.into(),
            rx,
            sample_rate as f32,
        )?,
        cpal::SampleFormat::U8 => build_output_stream::<u8>(
            &output_device,
            &output_config.into(),
            rx,
            sample_rate as f32,
        )?,
        cpal::SampleFormat::U16 => build_output_stream::<u16>(
            &output_device,
            &output_config.into(),
            rx,
            sample_rate as f32,
        )?,
        cpal::SampleFormat::U32 => build_output_stream::<u32>(
            &output_device,
            &output_config.into(),
            rx,
            sample_rate as f32,
        )?,
        cpal::SampleFormat::U64 => build_output_stream::<u64>(
            &output_device,
            &output_config.into(),
            rx,
            sample_rate as f32,
        )?,
        cpal::SampleFormat::F32 => build_output_stream::<f32>(
            &output_device,
            &output_config.into(),
            rx,
            sample_rate as f32,
        )?,
        cpal::SampleFormat::F64 => build_output_stream::<f64>(
            &output_device,
            &output_config.into(),
            rx,
            sample_rate as f32,
        )?,
        sample_format => panic!("Unsupported sample format: {}", sample_format),
    };

    // Start streams
    input_stream.play()?;
    output_stream.play()?;

    println!("   Processing started! Speak into your microphone...");
    println!("   Press Ctrl+C to stop");

    std::thread::park();

    Ok(())
}

fn build_input_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    tx: Sender<f32>,
) -> Result<cpal::Stream, cpal::BuildStreamError>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = config.channels as usize;

    let stream = device.build_input_stream(
        config,
        move |data: &[T], _: &cpal::InputCallbackInfo| {
            for frame in data.chunks(channels) {
                // Convert to mono by averaging channels
                let mut sample = 0.0f32;
                for &channel_sample in frame {
                    sample += f32::from_sample(channel_sample);
                }
                sample /= channels as f32;

                // Send to processing thread
                let _ = tx.try_send(sample);
            }
        },
        |err| eprintln!("Input stream error: {}", err),
        None,
    )?;

    Ok(stream)
}

fn build_output_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    rx: Receiver<f32>,
    sample_rate: f32,
) -> Result<cpal::Stream, cpal::BuildStreamError>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = config.channels as usize;

    // Create echo canceller (1024 taps ≈ 23ms at 44.1kHz, covers typical room acoustics)
    let mut echo_canceller = EchoCanceller::new(1024, 0.5);

    // Create voice activity detector for better echo cancellation
    let mut vad = VoiceActivityDetector::new(sample_rate, 0.0001);

    // Create granular engine and effects
    let mut engine = GranularEngine::new(
        sample_rate,
        2000.0, // 2 second buffer
        64,     // max grains
    );

    // Configure for dreamy/melancholic preset
    DreamyPreset::configure_engine(&mut engine);

    let mut effects = EffectsChain::new_dreamy(sample_rate);

    // Store previous speaker output for echo reference
    // FIXME: need to actually retrieve this from somewhere, totally useless right now
    let mut previous_speaker_output = 0.0;

    let stream = device.build_output_stream(
        config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            for frame in data.chunks_mut(channels) {
                // Get input sample
                let input_sample = rx.try_recv().unwrap_or(0.0);

                // Apply echo cancellation
                // This removes the speaker output that was picked up by the microphone
                let echo_cancelled = echo_canceller.process(input_sample, previous_speaker_output);

                // Detect voice activity and enable adaptation only when speaker output is present
                let speaker_active = vad.process(previous_speaker_output.abs());
                echo_canceller.set_adaptation_enabled(speaker_active);

                // Write to granular buffer
                engine.write_input(echo_cancelled);

                // Process granular synthesis
                let granular_output = engine.process();

                // Apply effects chain
                let processed = effects.process(granular_output);

                // Output to all channels
                let output_sample = T::from_sample(processed);
                for channel in frame.iter_mut() {
                    *channel = output_sample;
                }
            }
        },
        |err| eprintln!("Output stream error: {}", err),
        None,
    )?;

    Ok(stream)
}

fn get_mic(device_name: Option<String>, host: Host) -> Result<cpal::Device> {
    let mic = if let Some(name) = device_name {
        host.input_devices()?
            .find(|d| {
                d.description()
                    .map(|n| n.to_string().contains(&name))
                    .unwrap_or(false)
            })
            .expect("Could not find specified input device")
    } else {
        host.default_input_device().expect("No input device")
    };

    println!(
        "Using Input Device: {}",
        mic.description()
            .map(|d| d.to_string())
            .unwrap_or("Unknown".to_string())
    );
    Ok(mic)
}
