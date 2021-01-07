use std::{
    f32,
    thread,
    time::{Duration, Instant}
};


struct ModulationProfile {
    freq_hz: f32,
    min_val: i32,
    max_val: i32,

    previous_val: i32,
    current_val: i32
}

impl ModulationProfile {
    fn new(f_hz: f32, min_v: i32, max_v: i32) -> ModulationProfile {
        ModulationProfile {
            freq_hz: f_hz,
            min_val: min_v,
            max_val: max_v,
            current_val: max_v,
            previous_val: max_v
        }
    }

    fn update(&mut self, since_start: &Instant) {
        let dt = since_start.elapsed().as_millis() as f32;
        let ang_freq = self.freq_hz * 2.0 * f32::consts::PI as f32;

        self.previous_val = self.current_val;
        let val = self.min_val as f32 + ((self.max_val - self.min_val) as f32 * 0.5 * (1.0 + (dt * 0.001 * ang_freq).cos()));
        self.current_val = val.round() as i32;
    }
}

fn main() {
    let start = Instant::now();
    let mut mp = ModulationProfile::new(0.05, -50, 40);

    for i in 0..10 {
        mp.update(&start);
        println!("{} {} {}", i, mp.current_val, mp.previous_val);
        thread::sleep(Duration::from_millis(100));
    }
}
