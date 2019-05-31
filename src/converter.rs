// TODO: what about rounding error when calculating resampled length?

use std::ffi::CStr;
use std::fmt;
use std::io::{Read, Seek};

use failure::{Error, Fail};
use libc::{c_int, c_long};

// http://www.mega-nerd.com/SRC/api_misc.html#Converters
pub use libsamplerate_sys::SRC_LINEAR;
pub use libsamplerate_sys::SRC_SINC_BEST_QUALITY;
pub use libsamplerate_sys::SRC_SINC_FASTEST;
pub use libsamplerate_sys::SRC_SINC_MEDIUM_QUALITY;
pub use libsamplerate_sys::SRC_ZERO_ORDER_HOLD;

use crate::file::AudioFile;
use crate::file::Block as _;

pub struct Converter<R>
where
    R: Read + Seek,
{
    // Box to avoid recursive type
    file: Box<AudioFile<R>>,
    state: *mut libsamplerate_sys::SRC_STATE,
    // http://www.mega-nerd.com/SRC/api_misc.html#SRC_DATA
    data: libsamplerate_sys::SRC_DATA,
    samplerate: usize,
    buffer_in: Box<[f32]>,
    buffer_out: Box<[f32]>,
    current_block: Block,
}

impl<R> Converter<R>
where
    R: Read + Seek,
{
    pub fn new(file: AudioFile<R>, samplerate: usize) -> Result<Converter<R>, LibSamplerateError> {
        // TODO: specify type of converter
        // TODO: specify buffer size?
        let converter_type = SRC_SINC_BEST_QUALITY;
        let buffer_size = 1024;

        let channels = file.channels();
        let mut error: c_int = 0;

        let state = unsafe {
            // http://www.mega-nerd.com/SRC/api_full.html#Init
            libsamplerate_sys::src_new(converter_type as c_int, channels as c_int, &mut error)
        };
        if state.is_null() {
            return Err(LibSamplerateError(error));
        }

        // TODO: is initial src_set_ratio() necessary?
        //  int src_set_ratio (SRC_STATE *state, double new_ratio) ;

        // TODO: is checking src_is_valid_ratio() necessary?
        //  int src_is_valid_ratio (double ratio) ;  ??? public API ???

        let buffer_in = vec![0.0; buffer_size * channels];
        let mut buffer_out = vec![0.0; buffer_size * channels];
        let ptr_out = buffer_out.as_mut_ptr();

        Ok(Converter {
            data: libsamplerate_sys::SRC_DATA {
                data_in: buffer_in.as_ptr(),
                data_out: ptr_out,
                input_frames: 0,
                output_frames: 0,
                input_frames_used: 0,
                output_frames_gen: 0,
                end_of_input: 0,
                src_ratio: samplerate as f64 / file.samplerate() as f64,
            },
            file: Box::new(file),
            state,
            samplerate,
            current_block: Block {
                frames: 0,
                channels: (0..channels)
                    .map(|i| Channel {
                        ptr: unsafe { ptr_out.add(i) },
                        stride: channels,
                        len: 0,
                    })
                    .collect(),
            },
            // NB: Data will stay at the same memory address:
            buffer_in: buffer_in.into_boxed_slice(),
            buffer_out: buffer_out.into_boxed_slice(),
        })
    }

    pub fn samplerate(&self) -> usize {
        self.samplerate
    }

    pub fn channels(&self) -> usize {
        self.file.channels()
    }

    pub fn len(&self) -> usize {
        // TODO: is this correct? what about rounding errors?
        (self.file.len() as f64 * self.data.src_ratio) as usize
    }

    pub fn seek(&mut self, frame: usize) -> Result<(), Error> {
        // TODO: is this correct? what about rounding errors?
        self.file
            .seek((frame as f64 / self.data.src_ratio) as usize)?;
        // http://www.mega-nerd.com/SRC/api_full.html#Reset
        let result = unsafe { libsamplerate_sys::src_reset(self.state) };
        if result != 0 {
            return Err(LibSamplerateError(result).into());
        }
        self.data.end_of_input = 0;
        Ok(())
    }

    fn update_input_data<F>(&mut self, file: &mut F, frames: usize) -> Result<(), Error>
    where
        F: crate::file::ProvideBlocks,
    {
        let block = file.next_block(frames)?;
        if block.len() == 0 {
            self.data.end_of_input = 1;
        } else {
            let iterators = block.channel_iterators();
            let channels = iterators.len();
            for (i, source) in iterators.iter_mut().enumerate() {
                let start_idx = self.data.input_frames as usize * channels + i;
                let target = self.buffer_in[start_idx..].iter_mut().step_by(channels);
                for (a, b) in source.zip(target) {
                    *b = a
                }
            }
            self.data.input_frames += block.len() as c_long;
        }
        Ok(())
    }
}

// TODO: separate error type for SRC initialization?

#[derive(Debug, Fail)]
pub struct LibSamplerateError(pub i32);

// http://www.mega-nerd.com/SRC/api_misc.html#ErrorReporting
impl fmt::Display for LibSamplerateError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let msg = unsafe { libsamplerate_sys::src_strerror(self.0) };
        if msg.is_null() {
            write!(f, "Invalid error code: {}", self.0)
        } else {
            write!(f, "{}", unsafe { CStr::from_ptr(msg).to_str().unwrap() })
        }
    }
}

impl<R> Drop for Converter<R>
where
    R: Read + Seek,
{
    fn drop(&mut self) {
        unsafe {
            libsamplerate_sys::src_delete(self.state);
        }
    }
}

pub struct Block {
    frames: usize,
    channels: Box<[Channel]>,
}

impl crate::file::Block for Block {
    type Channel = Channel;

    fn channel_iterators(&mut self) -> &mut [Channel] {
        for channel in self.channels.iter_mut() {
            channel.len = self.frames;
        }
        &mut self.channels
    }

    fn len(&self) -> usize {
        self.frames
    }
}

pub struct Channel {
    ptr: *const f32,
    stride: usize,
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
            self.ptr = unsafe { self.ptr.add(self.stride) };
            Some(value)
        }
    }
}

impl<R> crate::file::ProvideBlocks for Converter<R>
where
    R: Read + Seek,
{
    type Block = Block;

    fn next_block(&mut self, max_len: usize) -> Result<&mut Block, Error> {
        let channels = self.file.channels();

        // We might have to call src_process() multiple times to get some data out
        loop {
            // Get new input data (and append to already existing input data)

            let frames = self.buffer_in.len() / channels - self.data.input_frames as usize;
            if frames > 0 {
                // TODO: there should probably be a safer way to do this?
                let mut file = std::mem::replace(&mut *self.file, unsafe { std::mem::zeroed() });
                match &mut file {
                    AudioFile::Vorbis(file) => self.update_input_data(file, frames)?,
                    AudioFile::Resampled(file) => self.update_input_data(file, frames)?,
                }
                std::mem::forget(std::mem::replace(&mut *self.file, file));
            }

            // Call libsamplerate to get new output data

            self.data.output_frames =
                std::cmp::min(self.buffer_out.len() / channels, max_len) as c_long;
            // http://www.mega-nerd.com/SRC/api_full.html#Process
            let result = unsafe { libsamplerate_sys::src_process(self.state, &mut self.data) };
            if result != 0 {
                return Err(LibSamplerateError(result).into());
            }

            // Copy unused input frames to beginning of input buffer

            self.data.input_frames -= self.data.input_frames_used;
            if self.data.input_frames > 0 {
                let used_samples = self.data.input_frames_used as usize * channels;
                let remaining_samples = self.data.input_frames as usize * channels;
                unsafe {
                    // TODO: Use slice::copy_within() once it is stabilized?
                    std::ptr::copy(
                        self.data.data_in.add(used_samples),
                        self.data.data_in as *mut _,
                        remaining_samples,
                    );
                }
            }

            // Create output block

            if self.data.output_frames_gen > 0
                || (self.data.input_frames == 0 && self.data.end_of_input == 1)
            {
                self.current_block.frames = self.data.output_frames_gen as usize;
                break Ok(&mut self.current_block);
            }
        }
    }
}
