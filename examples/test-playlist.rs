use std::fs;

use failure::Error;

use disk_streaming::{
    streamer::{load_audio_file, FileStreamer, PlaylistEntry},
};

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
        sources: Box::new([Some(0), Some(1)]),
    });

    let streamer = FileStreamer::new(playlist, 1024);

    // TODO: get some data

    Ok(())
}
