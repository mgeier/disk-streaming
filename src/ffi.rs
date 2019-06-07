// http://jakegoulding.com/rust-ffi-omnibus/objects/
// https://blog.eqrion.net/announcing-cbindgen/

use std::fs;

use failure::Error;

use crate::streamer::{load_audio_file, FileStreamer, PlaylistEntry};

// TODO: use catch_unwind()? https://doc.rust-lang.org/std/panic/fn.catch_unwind.html

// TODO: accept callback function for error reporting?

fn load_file_streamer(blocksize: usize, samplerate: usize) -> Result<FileStreamer, Error> {
    let channels = 4;
    let mut playlist = Vec::new();

    let file = fs::File::open("marimba.ogg")?;
    let file = load_audio_file(file, samplerate)?;
    playlist.push(PlaylistEntry {
        start: 0,
        end: Some(file.frames()),
        file,
        channels: Box::new([Some(0), Some(1)]),
    });

    let file = fs::File::open("marimba.ogg")?;
    let file = load_audio_file(file, samplerate)?;
    playlist.push(PlaylistEntry {
        start: 0 + 3 * 44_100,
        end: Some(file.frames() + 3 * 44_100),
        file,
        channels: Box::new([Some(2), Some(3)]),
    });

    Ok(FileStreamer::new(playlist, blocksize, channels))
}

#[no_mangle]
pub extern "C" fn file_streamer_new(
    blocksize: libc::size_t,
    samplerate: libc::size_t,
) -> *mut FileStreamer {
    if let Ok(streamer) = load_file_streamer(blocksize, samplerate) {
        Box::into_raw(Box::new(streamer))
    } else {
        std::ptr::null_mut()
    }
}

#[no_mangle]
pub extern "C" fn file_streamer_free(ptr: *mut FileStreamer) {
    if !ptr.is_null() {
        unsafe {
            Box::from_raw(ptr);
        }
    }
}

#[no_mangle]
pub extern "C" fn file_streamer_seek(ptr: *mut FileStreamer, frame: libc::size_t) -> bool {
    assert!(!ptr.is_null());
    let streamer = unsafe { &mut *ptr };
    streamer.seek(frame)
}

/// Return value of 0 means un-recoverable error
#[no_mangle]
pub unsafe extern "C" fn file_streamer_get_data(
    ptr: *mut FileStreamer,
    data: *const *mut f32,
) -> libc::size_t {
    assert!(!ptr.is_null());
    let streamer = &mut *ptr;
    streamer.get_data(std::slice::from_raw_parts(data, streamer.channels()))
}
