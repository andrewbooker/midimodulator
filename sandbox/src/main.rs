use std::sync::mpsc;
use std::thread;
use rtmidi::{RtMidiIn, RtMidiOut, RtMidiError};


struct Note {
    note: u8,
    velocity: u8
}

impl Note {
    pub fn from_midi_message(message: &[u8]) -> Note {
        Note {
            note: message[1],
            velocity: message[2]
        }
    }
}

fn main() -> Result<(), RtMidiError> {

    let input = RtMidiIn::new(Default::default())?;
    let input_ports = input.port_count()?;
    println!("{} MIDI input sources", input_ports);
    for port in 0..input_ports {
        println!("Input {}: {}", port + 1, input.port_name(port)?);
    }

    let output = RtMidiOut::new(Default::default())?;
    let output_ports = output.port_count()?;
    println!("{} MIDI output ports available.", output_ports);

    for port in 0..output_ports {
        println!("Output {}: {}", port + 1, output.port_name(port)?);
    }

    input.open_port(2, "RtMidi Input")?;
    output.open_port(2, "RtMidi Output")?;

    input.set_callback(|timestamp, message| {
        let n = Note::from_midi_message(&message);
        if n.velocity != 0 {
            println!("{:02x} {} {}", message[0], n.note, n.velocity);
            output.message(&[0x90, n.note, n.velocity]);
            output.message(&[0x80, n.note, 0]);
        }
    })?;

    input.ignore_types(true, true, true)?;

    println!("Reading MIDI input ...");
    
   
    let (cmd_stop_tx, cmd_stop_rx) = mpsc::channel();
    thread::spawn(move || {
        let g = getch::Getch::new();
        loop {
            let c: u8 = g.getch().unwrap();
            match c as char {
                'q' => {
                    cmd_stop_tx.send(()).unwrap();
                    break;
                },
                _ => {}
            }
        }
    });

    loop {
        match cmd_stop_rx.try_recv() {
            Ok(_) => {
                println!("stopping...");
                break;
            },
            _ => {}
        }
    }
    
    Ok(())
}
