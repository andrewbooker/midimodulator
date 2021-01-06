use std::f32;

fn main() {
    let max_updates_per_second = 10;
    let freq = 0.8;
    let ang_freq = freq * 2.0 * f32::consts::PI / max_updates_per_second as f32;

    for i in 0..max_updates_per_second {
        println!("{} {}", i, (i as f32 * ang_freq).cos());
    }
}
