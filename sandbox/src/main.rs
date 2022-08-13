use std::io::{stdin, Read};
use rtmidi::{RtMidiIn, RtMidiOut, RtMidiError};

fn main() -> Result<(), RtMidiError> {
    // Initialise MIDI input
    let input = RtMidiIn::new(Default::default())?;

    // Get number of input ports
    let input_ports = input.port_count()?;
    println!("There are {} MIDI input sources available.", input_ports);

    // List input ports
    for port in 0..input_ports {
        println!("\tInput Port #{}: {}", port+1, input.port_name(port)?);
    }

    // Initialise MIDI output
    let output = RtMidiOut::new(Default::default())?;

    // Get number of output ports
    let output_ports = output.port_count()?;
    println!("There are {} MIDI output ports available.", output_ports);

    // List output ports
    for port in 0..output_ports {
        println!("\tOutput Port #{}: {}", port+1, output.port_name(port)?);
    }
    
    
    // Open first available port
    input.open_port(2, "RtMidi Input")?;

    // Set our callback function.  This should be done immediately after
    // opening the port to avoid having incoming messages written to the
    // queue.
    input.set_callback(|timestamp, message| {
        for (index, byte) in message.iter().enumerate() {
            println!("Byte {} = 0x{:02x}, ", index, byte);
        }
    })?;

    input.ignore_types(true, true, true)?;

    println!("Reading MIDI input ...");
    stdin().read(&mut [0]).unwrap();
    
    Ok(())
}
