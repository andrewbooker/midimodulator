extern crate libc;

mod korg;
mod midi;
mod d110;
mod utils;
mod modulation;

use crate::modulation::{
    SysExComposer,
    PairedUpdater,
    StepInterval,
    SweepState,
    Selector
};
use crate::d110::{
    init_d110,
    init_timbre,
    set_up_tone,
    PARTIAL_SPEC,
    D110SysEx
};
use crate::korg::{
    KorgProgramSysEx,
    KorgInitSysEx,
    KorgSingleParamSysEx,
    PROGRAM_SPEC,
    OSC_SPEC,
    KorgOscSelector,
    KorgEffectSelector
};
use crate::midi::{MidiMessage, MidiOut, MidiOutDevices};
use std::{
    thread,
    time::{Duration, Instant},
    sync::{mpsc, mpsc::{Sender, Receiver}},
    io::{prelude::*, BufReader},
    net::TcpListener,
    collections::HashMap
};


struct DummySelector;

impl DummySelector {
    fn new() -> DummySelector {
        DummySelector {}
    }
}

impl Selector for DummySelector {
    fn next1(&mut self) {}
    fn next2(&mut self) {}

    fn val(&self, _: u8) -> u16 { 0 }
}


struct TimeBasedInterval {
    start: Instant
}

impl TimeBasedInterval {
    fn new() -> TimeBasedInterval {
        TimeBasedInterval {
            start: Instant::now()
        }
    }
}

impl StepInterval for TimeBasedInterval {
    fn interval(&self) -> f32 {
        self.start.elapsed().as_millis() as f32
    }
}

struct FixedEquivalentMillisInterval {
    int: u32
}

impl FixedEquivalentMillisInterval {
    fn new(int: u32) -> FixedEquivalentMillisInterval {
        FixedEquivalentMillisInterval {
            int
        }
    }
}

impl StepInterval for FixedEquivalentMillisInterval {
    fn interval(&self) -> f32 {
        self.int as f32
    }
}

const NUM_D110_PARTS: usize = 2;

fn update_d110(updater: &mut PairedUpdater, d110_midi_out: &mut MidiOut) {
    let mut dummy_1 = DummySelector::new();
    let mut dummy_2 = DummySelector::new();

    let tones: [& mut D110SysEx; NUM_D110_PARTS] = [
        &mut set_up_tone(1),
        &mut set_up_tone(2)
    ];

    let prefixes = ["A_1", "B_3", "C_2", "D_4"];
    for t in 0..NUM_D110_PARTS {
        for p in prefixes {
            updater.update(tones[t], &mut dummy_1, &mut dummy_2, &PARTIAL_SPEC, Some(&*format!("tone{}_partial{}", t + 1, p)));
        }
    }

    updater.sweep_alternator();

    for t in 0..NUM_D110_PARTS {
        let v = tones[t].to_send();
        d110_midi_out.send_sys_ex(&v);
    }

    println!("--");
    for (key, val) in &updater.sweep_state {
        println!("{}: {}", key, val.val);
    }
}


fn modulate_d110(device_number: i32) {
    let mut d110_midi_out = MidiOut::using_device(device_number);
    let d110_init = init_d110();
    d110_midi_out.send_sys_ex(&d110_init.to_send());
    for t in 1..9 {
        println!("sending timbre {}", t);
        d110_midi_out.send_sys_ex(&init_timbre(t).to_send());
    }
    for t in 1..9 {
        d110_midi_out.send_sys_ex(&set_up_tone(t).to_send());
    }
    println!("D110 init sent");

    let listener = TcpListener::bind("0.0.0.0:7878").unwrap();
    println!("tcp listener started on port 7878");

    let mut count: u32 = 0;
    for stream in listener.incoming() {
        let mut stream = stream.unwrap();
        let buf_reader = BufReader::new(&mut stream);
        let http_request: Vec<_> = buf_reader
            .lines()
            .map(|result| result.unwrap())
            .take_while(|line| !line.is_empty())
            .collect();

        count += 1;

        let status_line = "HTTP/1.1 200 OK";
        let contents = json::stringify(vec![0]);
        let length = contents.len();
        let response = format!("{status_line}\r\nContent-Type: application/json; charset=UTF-8\r\nContent-Length: {length}\r\n\r\n{contents}");
        stream.write_all(response.as_bytes()).unwrap();

        let interval = FixedEquivalentMillisInterval::new(1000 * count);
        let mut updater = PairedUpdater::new(&interval);
        update_d110(&mut updater, &mut d110_midi_out);
    }
}


fn modulate_korg<C>(cmd_dump_rx: &Receiver<C>, res_tx: &Sender<HashMap<std::string::String, SweepState>>, first_tx: &Sender<i32>) {
    let mut port = serialport::new("/dev/ttyS0", 38400)
                    .timeout(Duration::from_millis(1000))
                    .open()
                    .expect("Failed to open port");

    let interval = TimeBasedInterval::new();
    let mut updater = PairedUpdater::new(&interval);
    let mut effect_selector = KorgEffectSelector::new();
    let mut osc_selector = KorgOscSelector::new();

    let today = utils::today();
    let mut first = true;

    loop {
        let mut kpsx = KorgProgramSysEx::new();
        kpsx.name(&today);

        let eff1_updater = &effect_selector.eff1.updater;
        let eff2_updater = &effect_selector.eff2.updater;
        let pre_eff = &effect_selector.pre_eff();

        updater.update(&mut kpsx, &mut osc_selector, &mut effect_selector, &PROGRAM_SPEC, None);
        updater.update(&mut kpsx, &mut osc_selector, &mut effect_selector, &OSC_SPEC, Some("osc1"));
        updater.update(&mut kpsx, &mut osc_selector, &mut effect_selector, &OSC_SPEC, Some("osc2"));
        updater.update(&mut kpsx, &mut osc_selector, &mut effect_selector, pre_eff, None);
        updater.update(&mut kpsx, &mut osc_selector, &mut effect_selector, eff1_updater, Some("eff1"));
        updater.update(&mut kpsx, &mut osc_selector, &mut effect_selector, eff2_updater, Some("eff2"));
        updater.sweep_alternator();

        port.write(&kpsx.data).expect("Write failed!");
        thread::sleep(Duration::from_millis(100));

        match cmd_dump_rx.try_recv() {
            Ok(_) => {
                res_tx.send(updater.sweep_state.clone()).unwrap();
            },
            _ => {}
        }
        if first {
            first_tx.send(0).unwrap();
            first = false;
            thread::sleep(Duration::from_millis(100));
        }
    }
}


fn main() {
    let d110_number = MidiOutDevices::index_of("4i4o MIDI 4").unwrap();
    let korg_number = MidiOutDevices::index_of("4i4o MIDI 3").unwrap();
    println!("D110 port {}", d110_number);
    println!("Korg port {}", korg_number);

    thread::spawn(move || { modulate_d110(d110_number); });

    let mut midi_out = MidiOut::using_device(korg_number);
    midi_out.send_sys_ex(&KorgInitSysEx::new(0x02).data); // select prog
    midi_out.send(&MidiMessage::program(33, korg::CHANNEL)); // select 33
    thread::sleep(Duration::from_millis(100));

    midi_out.send_sys_ex(&KorgInitSysEx::new(0x03).data); // edit prog
    midi_out.send_sys_ex(&KorgSingleParamSysEx::new(0, 1).data); // oscillator mode: Double, on UI, otherwise the screen value overrides th sysEx

    let ports = serialport::available_ports().expect("No ports found!");
    for p in ports {
        println!("{}", p.port_name);
    }

    let (cmd_dump_tx, cmd_dump_rx) = mpsc::channel();
    let (cmd_stop_tx, cmd_stop_rx) = mpsc::channel();
    let (res_tx, res_rx) = mpsc::channel();
    let (first_tx, first_korg_rx) = mpsc::channel();

    thread::spawn(move || { modulate_korg(&cmd_dump_rx, &res_tx, &first_tx); });

    thread::spawn(move || {
        let g = getch::Getch::new();
        loop {
            let c: u8 = g.getch().unwrap();
            match c as char {
                'l' => {
                    cmd_dump_tx.send(()).unwrap();
                    for res in &res_rx {
                        for (key, val) in &res {
                            println!("{}: {}", key, val.val);
                        }
                        break;
                    }
                },
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
            _ => thread::sleep(Duration::from_millis(50))
        }
        match first_korg_rx.try_recv() {
            Ok(0) => {
                midi_out.send_sys_ex(&KorgInitSysEx::new(0x02).data); // select prog (deselect edit, otherwise the oscillators don't change
                println!("first korg modulation sent");
            },
            _ => thread::sleep(Duration::from_millis(50))
        }
    }
    thread::sleep(Duration::from_millis(2000));
}
