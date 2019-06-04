use std::fs;

use failure::Error;

use disk_streaming::{
    file::AudioFile,
    streamer::{FileStreamer, PlaylistEntry},
};

fn main() -> Result<(), Error> {
    // TODO: specify blocksize and samplerate!

    let mut playlist = Vec::new();

    // TODO: get "end" from file length?

    let file = fs::File::open("marimba.ogg")?;
    let file = AudioFile::with_samplerate(file, 44_100)?;
    //let file = AudioFile::with_samplerate(file, 48_000)?;

    playlist.push(PlaylistEntry {
        start: 0,
        end: Some(file.len()),
        file,
        sources: Box::new([0, 1]),
    });

    let streamer = FileStreamer::new(playlist);

    // TODO: get some data

    Ok(())
}
