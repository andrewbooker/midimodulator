extern crate libc;

use std::{
    f32,
    thread,
    time::{Duration, Instant},
    os::raw::{c_char, c_int, c_void},
    ffi::CStr
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


#[repr(C)]
pub struct PmDeviceInfo {
    pub struct_version: c_int,
    pub interf: *const c_char,
    pub name: *const c_char,
    pub input: c_int,
    pub output: c_int,
    pub opened: c_int,
}

#[link(name = "portmidi")]
extern "C" {
    pub fn Pm_Initialize() -> c_int;
    pub fn Pm_Terminate() -> c_int;
    pub fn Pm_CountDevices() -> c_int;
    pub fn Pm_GetDeviceInfo(id: c_int) -> *const PmDeviceInfo;
    pub fn Pm_OpenOutput(stream: *const *const c_void,
                         outputDeviceId: c_int,
                         inputDriverInfo: *const c_void,
                         bufferSize: i32,
                         time_proc: *const c_void,
                         time_info: *const c_void,
                         latency: i32) -> c_int;
}

fn to_string(s: *const c_char) -> String {
    unsafe { CStr::from_ptr(s) }.to_str().ok().unwrap().to_owned()
}

fn main() {
    let start = Instant::now();
    let mut mp = ModulationProfile::new(0.05, -50, 40);

    for i in 0..10 {
        mp.update(&start);
        println!("{} {} {}", i, mp.current_val, mp.previous_val);
        thread::sleep(Duration::from_millis(100));
    }

    unsafe { Pm_Initialize() };
    let c = unsafe { Pm_CountDevices() };
    println!("{} devices found", c);

    let info_ptr = unsafe { Pm_GetDeviceInfo(2) };
    println!("{}", unsafe { (*info_ptr).output });
    println!("{}", to_string(unsafe { (*info_ptr).name }));

    unsafe { Pm_Terminate() };
}
