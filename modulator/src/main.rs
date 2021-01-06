use std::{
    f32,
    thread,
    time::{Duration, Instant}
};

fn output_from(start: Instant) {
    let freq = 0.8;

    let dt = start.elapsed().as_millis() as f32;
    let ang_freq =  freq * 2.0 * f32::consts::PI as f32;

    println!("{} {}", dt, (dt * 0.001 * ang_freq).cos());
}

fn main() {
    let start = Instant::now();

    for i in 0..10 {
        output_from(start);
        thread::sleep(Duration::from_millis(100));
    }

}
