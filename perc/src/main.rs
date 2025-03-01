use std::fs::File;
use std::io::BufReader;
use rodio::{Decoder, OutputStream, source::Source};
use rodio::*;
use rodio::cpal::traits::{HostTrait, DeviceTrait};



fn listHostDevices() {
    let host = cpal::default_host();
    let devices = host.output_devices().unwrap();
    for device in devices { 
        let dev: rodio::Device = device.into();
        let devName: String = dev.name().unwrap();
        if devName.contains("hdmi") {
            println!(" # Device : {}", devName);
        }
    }
}

fn main() {

    listHostDevices();



    let file_name = "/home/abooker/Music/test/perc/00001.wav";

    // Get a output stream handle to the default physical sound device
    let (_stream, stream_handle) = OutputStream::try_default().unwrap();
    
    // Load a sound from a file, using a path relative to Cargo.toml
    let file = BufReader::new(File::open(file_name).unwrap());
    // Decode that sound file into a source
    let source = Decoder::new(file).unwrap();
    // Play the sound directly on the device
    stream_handle.play_raw(source.convert_samples());

    // The sound plays in a separate audio thread,
    // so we need to keep the main thread alive while it's playing.
    std::thread::sleep(std::time::Duration::from_secs(5));
}
