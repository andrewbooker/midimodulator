use std::{
    f32,
    thread,
    time::{Duration, Instant}
};

fn output_from(start: Instant) {
    let freq_hz = 0.05;
    let max_val: f32 = 40 as f32;
    let min_val: f32 = -50 as f32;

    let dt = start.elapsed().as_millis() as f32;
    let ang_freq = freq_hz * 2.0 * f32::consts::PI as f32;

    let val = min_val + ((max_val - min_val) * 0.5 * (1.0 + (dt * 0.001 * ang_freq).cos()));
    println!("{} {}", dt, val.round() as i32);
}

fn main() {
    let start = Instant::now();

    for i in 0..10 {
        output_from(start);
        thread::sleep(Duration::from_millis(100));
    }
}
