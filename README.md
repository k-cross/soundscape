# Soundscape - Reactive Procedural Audio Engine

A rust-based procedural audio thing that generates soundscapes reacting to microphone input and keyboard activity. It uses `tunes` and `fundsp`/`rundsp` for synthesis while `cpal` is used for mic input and `crossterm` is used for keyboard semantics. This is really janky, but kinda fun.

## Features

- procedural audio generation: Uses `fundsp` and `tunes` to create dsp graphs
- microphone input: ambient noise level influences the soundscape
- keyboard input: typing adds _energy_ and variability to the system
- modes:
    - `reactive`: a drone synthesizer that becomes brighter, more chaotic, and louder with input
    - `hybrid`: a pentatonic minor arpeggio that speeds up and has random octave jumps/note choices based on input _energy_

## Usage

Run the project using `cargo`, uses `stable` rust:

```sh
cargo run --release -- -h
Usage: soundscape [OPTIONS]

Options:
  -m, --mode <MODE>      Operation mode [default: hybrid] [possible values: reactive, hybrid]
      --list-devices     List available input devices and exit
      --device <DEVICE>  Name (or substring) of the input device to use
  -h, --help             Print help
  -V, --version          Print version
```

### Modes

- `reactive` (Default): Drone mode.
- `hybrid`: Arpeggiator mode.

### Options

- `--list-devices`: List available audio input devices.
- `--device <NAME>`: Specify an input device by name (or substring).

### Controls

- microphone: make noise to increase _energy_
- keyboard: type keys to add bursts of _energy_
    - ctrl+c: exit
