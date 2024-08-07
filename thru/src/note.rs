use std::time::Instant;

pub struct Note {
    pub note: u8,
    pub velocity: u8
}

impl Note {
    pub fn from_midi_message(message: &[u8]) -> Note {
        Note {
            note: message[1],
            velocity: message[2]
        }
    }
}

pub const NOTE_HISTORY: usize = 8;
pub type PlayedNote = (u8, u8, Instant);

pub struct NoteStats {
    pub received: [PlayedNote; NOTE_HISTORY],
    pub last_dropped: PlayedNote
}

impl NoteStats {
    pub fn new() -> Self {
        Self {
            received: [(0, 0, Instant::now()); NOTE_HISTORY],
            last_dropped: (0, 0, Instant::now())
        }
    }

    pub fn last(&self) -> PlayedNote {
        self.received[NOTE_HISTORY - 1]
    }

    pub fn look_back(&self, b: u8) -> PlayedNote {
        self.received[NOTE_HISTORY - usize::from(b)]
    }

    pub fn sending_note_on(&mut self, n: u8, c: u8) {
        for i in 1..NOTE_HISTORY {
            self.received[i - 1] = self.received[i];
        }
        self.received[NOTE_HISTORY - 1] = (n, c, Instant::now());
    }

    pub fn drop(&mut self, n: u8) {
        self.last_dropped = (n, self.last().1, Instant::now());
    }
}


// Scale

pub type Mode = [u8; 6];
const SCALE_LENGTH: usize = 8;

pub struct Scale {
    pub notes: [u8; SCALE_LENGTH]
}

impl Scale {
    pub fn from(tonic: u8, mode: &Mode) -> Scale {
        let mode_len: u8 = mode.len() as u8;

        let mut notes = [0; SCALE_LENGTH];
        let mut octaves: u8 = 0;
        let mut base: u8 = tonic;

        for n in 0..SCALE_LENGTH as u8 {
            if (n % (mode_len + 1)) == 0 {
                base = tonic + (octaves * 12) as u8;
                octaves += 1;
            } else {
                let idx = (n - octaves) % mode_len;
                base += mode[idx as usize];
            }
            notes[n as usize] = base;
        }
        Scale {
            notes
        }
    }

    pub fn at(&self, idx: u8) -> u8 {
        let len = self.notes.len() as u8;
        if len > idx {
            return self.notes[idx as usize];
        }
        return self.at(idx + 1 - len) + 12;
    }

    pub fn ordinal_of(&self, note: u8) -> u8 {
        let found = self.notes.iter().position(|&n| n == note);
        if found.is_some() {
            return found.unwrap() as u8;
        }
        if note < self.at(0) {
            return self.ordinal_of(note + 12);
        }
        if note > self.at(7) {
            return self.ordinal_of(note - 12);
        }
        std::panic::panic_any("Note not in scale");
    }
}


#[cfg(test)]
mod test_stats {
    use super::NoteStats;

    #[test]
    fn last_note_is_zero_if_no_note_yet_played() {
        assert_eq!(NoteStats::new().last().0, 0);
    }

    #[test]
    fn records_last_note_on_received() {
        let mut stats = NoteStats::new();

        stats.sending_note_on(1, 0);

        assert_eq!(stats.last().0, 1);
    }
}


#[cfg(test)]
mod test_scale {
    use super::Scale;

    #[test]
    fn can_construct_from_tonic_and_mode() {
        let scale = Scale::from(60, &[2, 2, 1, 2, 2, 2]);

        assert_eq!(scale.notes, [60, 62, 64, 65, 67, 69, 71, 72]);
        assert_eq!(scale.at(0), 60);
    }

    #[test]
    fn can_access_scale_element_beyond_octave() {
        let scale = Scale::from(60, &[2, 2, 1, 2, 2, 2]);

        assert_eq!(scale.at(0), 60);
        assert_eq!(scale.at(7), 72);
        assert_eq!(scale.at(8), 74);
        assert_eq!(scale.at(9), 76);
        assert_eq!(scale.at(10), 77);
    }

    #[test]
    fn can_calculate_base_ordinal_from_note_within_scale() {
        let scale = Scale::from(60, &[2, 2, 1, 2, 2, 2]);

        assert_eq!(scale.at(0), 60);
        assert_eq!(scale.ordinal_of(60), 0);
        assert_eq!(scale.ordinal_of(62), 1);
        assert_eq!(scale.ordinal_of(72), 7);
        assert_eq!(scale.ordinal_of(74), 1);
        assert_eq!(scale.ordinal_of(76), 2);
        assert_eq!(scale.ordinal_of(88), 2);
        assert_eq!(scale.ordinal_of(59), 6);
        assert_eq!(scale.ordinal_of(47), 6);
    }
}
