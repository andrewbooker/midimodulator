#MIDI Modulator

There are two components to this project, both of which I developed to make the fullest use possible out of legacy MIDI sound modules I still own, so as to make improvisation sessions more interesting. I have built previous versions of these ideas in C++ or python, and am using this project as an excuse to learn RUST.

Both cases are highly specific to my setup and musical approach to improvising. While these sub-projects may provide a useful how-to reference for some aspects of MIDI control, I do not claim they will be generally useful for playing organised or composed music. 

##Modulator
The `modulator` project sends midi System Exclusive control signals to MIDI sound modules. The purpose is the continuously change program settings so that the program will sound different each time it is hit, and if possible, the sound will change while it is playing. So far I have written controllers for two sound modules that I own:

- Roland D110
- Korg 05R/W

The korg is a rare example of a legacy sound module that is especially suited to this approach of changing sounds in real time, as it has a separate dedicated serial (MIDI) input that seems to have been included in the hardware for processing signal control systems without them colliding with playing signals. The Roland D110 is typical of a sound module that does not.

##Thru
The `thru` project directs MIDI in signals to a MIDI out, with some transformation or other.

