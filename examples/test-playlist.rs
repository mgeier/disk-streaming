use std::fs;
use std::thread;
use std::time::Duration;

use failure::Error;

use disk_streaming::streamer::{load_audio_file, FileStreamer, PlaylistEntry};

fn main() -> Result<(), Error> {
    // TODO: specify blocksize and samplerate!

    let mut playlist = Vec::new();

    // TODO: get "end" from file length?

    let file = fs::File::open("marimba.ogg")?;
    let file = load_audio_file(file, 44_100)?;
    //let file = AudioFile::with_samplerate(file, 48_000)?;

    playlist.push(PlaylistEntry {
        start: 0,
        end: Some(file.frames()),
        file,
        channels: Box::new([Some(0), Some(1)]),
    });

    let blocksize = 1024;
    let channels = 4;

    let mut streamer = FileStreamer::new(playlist, blocksize, channels);

    let mut data: Vec<Vec<_>> = (0..streamer.channels()).map(|_| vec![0f32; blocksize]).collect();

    let pointers: Vec<*mut f32> = data.iter_mut().map(|v| v.as_mut_ptr()).collect();

    for i in 1.. {
        print!("seek attempt {} ... ", i);
        if streamer.seek(100) {
            println!("success");
            break
        }
        println!("failed");
        thread::sleep(Duration::from_millis(1));
    }

    let result = unsafe { streamer.get_data(&pointers) };

    println!("got {} frames of data", result);

    Ok(())
}
