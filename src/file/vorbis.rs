use std::fmt;
use std::io::{self, Read, Seek};

use failure::{Error, Fail};
use libc::{c_int, c_long, c_void};
use ogg_sys::ogg_int64_t;

const EIO: errno::Errno = errno::Errno(5);

/// https://xiph.org/vorbis/doc/vorbisfile/reference.html
pub struct File<R>
where
    R: Read + Seek,
{
    // https://xiph.org/vorbis/doc/vorbisfile/OggVorbis_File.html
    ov_struct: vorbisfile_sys::OggVorbis_File,
    #[allow(dead_code)]
    reader: Box<R>, // A Box is used to get a stable memory address to pass as "datasource"
    samplerate: usize,
    channels: usize,
    frames: usize,
    current_block: Block,
}

unsafe impl<R: Read + Seek + Send> Send for File<R> {}

impl<R> Drop for File<R>
where
    R: Read + Seek,
{
    fn drop(&mut self) {
        unsafe {
            // https://xiph.org/vorbis/doc/vorbisfile/ov_clear.html
            vorbisfile_sys::ov_clear(&mut self.ov_struct);
        }
    }
}

// Callback functions in large parts inspired by https://github.com/tomaka/vorbis-rs

extern "C" fn read_func<R>(
    ptr: *mut c_void,
    size: libc::size_t,
    nmemb: libc::size_t,
    datasource: *mut c_void,
) -> libc::size_t
where
    R: Read,
{
    let ptr = ptr as *mut u8;
    assert!(!datasource.is_null());
    let reader = unsafe { &mut *(datasource as *mut R) };
    let buffer = unsafe { std::slice::from_raw_parts_mut(ptr, (size * nmemb) as usize) };
    loop {
        match reader.read(buffer) {
            Ok(bytes) => return bytes,
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => {
                errno::set_errno(EIO);
                return 0;
            }
        }
    }
}

extern "C" fn seek_func<R>(datasource: *mut c_void, offset: ogg_int64_t, whence: c_int) -> c_int
where
    R: Seek,
{
    assert!(!datasource.is_null());
    let reader = unsafe { &mut *(datasource as *mut R) };
    let result = match whence {
        libc::SEEK_SET => reader.seek(io::SeekFrom::Start(offset as u64)),
        libc::SEEK_CUR => reader.seek(io::SeekFrom::Current(offset)),
        libc::SEEK_END => reader.seek(io::SeekFrom::End(offset)),
        w => panic!("Invalid value for \"whence\": {}", w),
    };
    result.map(|v| v as c_int).unwrap_or(-1)
}

extern "C" fn tell_func<R>(datasource: *mut c_void) -> c_long
where
    R: Seek,
{
    assert!(!datasource.is_null());
    let reader = unsafe { &mut *(datasource as *mut R) };
    reader
        .seek(io::SeekFrom::Current(0))
        .map(|v| v as c_long)
        .unwrap_or(-1)
}

extern "C" fn close_func(_datasource: *mut c_void) -> c_int {
    // Nothing to do here, "reader" is cleaned up automatically
    0
}

#[derive(Debug, Fail)]
pub struct LibVorbisError(pub i32);

impl fmt::Display for LibVorbisError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use vorbis_sys::*;
        write!(
            f,
            // TODO: display name of constant, e.g. "OV_EBADHEADER"
            "Vorbis error: {}",
            match self.0 {
                OV_EREAD => "A read from media returned an error",
                OV_ENOTVORBIS => "Bitstream does not contain any Vorbis data",
                OV_EVERSION => "Vorbis version mismatch",
                OV_EBADHEADER => "Invalid Vorbis bitstream header",
                OV_EFAULT => "Internal logic fault; indicates a bug or heap/stack corruption",
                OV_EINVAL => "Invalid argument value",
                OV_HOLE => {
                    "Interruption in the data (one of: garbage between pages, \
                     loss of sync followed by recapture, or a corrupt page)"
                }
                OV_EBADLINK => "Invalid stream section, or the requested link is corrupt",
                OV_ENOSEEK => "Bitstream is not seekable",
                e => panic!("Unknown Vorbis error code: {}", e),
            }
        )
    }
}

#[derive(Debug, Fail)]
pub enum OpenError {
    Vorbis(#[cause] LibVorbisError),
    ChangingRate,
    ChangingChannels,
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Error opening Vorbis file: ")?;
        use OpenError::*;
        match self {
            // Special case for ov_pcm_total():
            Vorbis(LibVorbisError(vorbis_sys::OV_EINVAL)) => write!(
                f,
                "Unable to determine total duration (the bitstream is unseekable?)"
            ),
            Vorbis(e) => e.fmt(f),
            ChangingRate => write!(f, "Changing sampling rate within a file is not supported"),
            ChangingChannels => write!(
                f,
                "Changing the number of channels within a file is not supported"
            ),
        }
    }
}

impl<R> File<R>
where
    R: Read + Seek,
{
    // TODO: allow using only a subset of all channels!

    pub fn new(reader: R) -> Result<File<R>, OpenError> {
        // https://xiph.org/vorbis/doc/vorbisfile/ov_callbacks.html
        let callbacks = vorbisfile_sys::ov_callbacks {
            read_func: read_func::<R>,
            seek_func: seek_func::<R>,
            close_func: close_func,
            tell_func: tell_func::<R>,
        };
        // TODO: use MaybeUninit::uninit()?
        // TODO: or use zeroed()?
        let mut ov_struct: vorbisfile_sys::OggVorbis_File = unsafe { std::mem::uninitialized() };
        //let mut ov_struct: vorbisfile_sys::OggVorbis_File = unsafe {
        //    std::mem::MaybeUninit::uninit() };
        let mut reader = Box::new(reader);
        let result: c_int = unsafe {
            // https://xiph.org/vorbis/doc/vorbisfile/ov_open_callbacks.html
            vorbisfile_sys::ov_open_callbacks(
                &mut *reader as *mut R as *mut c_void,
                &mut ov_struct,
                std::ptr::null(),
                0,
                callbacks,
            )
        };
        if result != 0 {
            return Err(OpenError::Vorbis(LibVorbisError(result)));
        }
        // TODO: use MaybeUninit::assume_init()?
        assert!(ov_struct.links > 0);
        let info = unsafe { &*ov_struct.vi };
        let rate = info.rate;
        let channels = info.channels;
        for i in 1..ov_struct.links as usize {
            let info = unsafe { &*ov_struct.vi.add(i) };
            if info.rate != rate {
                return Err(OpenError::ChangingRate);
            }
            if info.channels != channels {
                return Err(OpenError::ChangingChannels);
            }
        }
        let frames = unsafe { vorbisfile_sys::ov_pcm_total(&mut ov_struct, -1) };
        if frames < 0 {
            Err(OpenError::Vorbis(LibVorbisError(frames as i32)))
        } else {
            Ok(File {
                ov_struct,
                reader,
                samplerate: rate as usize,
                channels: channels as usize,
                frames: frames as usize,
                current_block: Block {
                    ptr: std::ptr::null_mut(),
                    frames: 0,
                    channels: (0..channels)
                        .map(|_| Channel {
                            ptr: std::ptr::null_mut(),
                            len: 0,
                        })
                        .collect(),
                },
            })
        }
    }
}

impl<R> super::AudioFileBasics for File<R>
where
    R: Read + Seek,
{
    fn seek(&mut self, frame: usize) -> Result<(), Error> {
        // https://xiph.org/vorbis/doc/vorbisfile/ov_pcm_seek.html
        let result =
            unsafe { vorbisfile_sys::ov_pcm_seek(&mut self.ov_struct, frame as ogg_int64_t) };
        if result == 0 {
            Ok(())
        } else {
            Err(LibVorbisError(result).into())
        }
    }

    fn samplerate(&self) -> usize {
        self.samplerate
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn frames(&self) -> usize {
        self.frames
    }
}

impl<R> super::AudioFileBlocks for File<R>
where
    R: Read + Seek,
{
    type Block = Block;

    fn next_block(&mut self, max_frames: usize) -> Result<&mut Block, Error> {
        let mut current_section: c_int = 0;
        let result: c_long = unsafe {
            // https://xiph.org/vorbis/doc/vorbisfile/ov_read_float.html
            vorbisfile_sys::ov_read_float(
                &mut self.ov_struct,
                &mut self.current_block.ptr,
                max_frames as c_int,
                &mut current_section, // Result is ignored
            )
        };
        if result < 0 {
            return Err(LibVorbisError(result as i32).into());
        }
        self.current_block.frames = result as usize;
        Ok(&mut self.current_block)
    }
}

pub struct Block {
    ptr: *mut *mut f32,
    frames: usize,
    channels: Box<[Channel]>,
}

impl super::Block for Block {
    type Channel = Channel;

    fn channel_iterators(&mut self) -> &mut [Channel] {
        for i in 0..self.channels.len() {
            let channel = &mut self.channels[i];
            channel.ptr = unsafe { *self.ptr.add(i) };
            channel.len = self.frames;
        }
        &mut self.channels
    }

    fn frames(&self) -> usize {
        self.frames
    }
}

pub struct Channel {
    ptr: *mut f32,
    len: usize,
}

impl Iterator for Channel {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.len == 0 {
            None
        } else {
            let value = unsafe { *self.ptr };
            self.len -= 1;
            self.ptr = unsafe { self.ptr.add(1) };
            Some(value)
        }
    }

    // TODO: size_hint()?
}
