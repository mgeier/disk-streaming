use std::ffi::CStr;
use std::fmt;

use failure::{Error, Fail};
use libc::{c_int, c_long};

use super::{AudioFileBasics, AudioFileBlocks};

// http://www.mega-nerd.com/SRC/api_misc.html#Converters
pub use libsamplerate_sys::SRC_LINEAR;
pub use libsamplerate_sys::SRC_SINC_BEST_QUALITY;
pub use libsamplerate_sys::SRC_SINC_FASTEST;
pub use libsamplerate_sys::SRC_SINC_MEDIUM_QUALITY;
pub use libsamplerate_sys::SRC_ZERO_ORDER_HOLD;

pub struct Converter<F>
where
    F: AudioFileBasics + AudioFileBlocks,
{
    file: F,
    state: *mut libsamplerate_sys::SRC_STATE,
    // http://www.mega-nerd.com/SRC/api_misc.html#SRC_DATA
    data: libsamplerate_sys::SRC_DATA,
    samplerate: usize,
    buffer_in: Box<[f32]>,
    buffer_out: Box<[f32]>,
    current_block: Block,
}

unsafe impl<F: AudioFileBasics + AudioFileBlocks + Send> Send for Converter<F> {}

impl<F> Converter<F>
where
    F: AudioFileBasics + AudioFileBlocks,
{
    pub fn new(file: F, samplerate: usize) -> Result<Converter<F>, LibSamplerateError> {
        // TODO: specify type of converter
        let converter_type = SRC_SINC_BEST_QUALITY;
        // TODO: specify buffer size?
        let buffer_size = 2048;

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

        Ok(Converter {
            data: libsamplerate_sys::SRC_DATA {
                data_in: buffer_in.as_ptr(),
                data_out: buffer_out.as_mut_ptr(),
                input_frames: 0,
                output_frames: 0,
                input_frames_used: 0,
                output_frames_gen: 0,
                end_of_input: 0,
                src_ratio: samplerate as f64 / file.samplerate() as f64,
            },
            file,
            state,
            samplerate,
            current_block: Block {
                ptr: buffer_out.as_ptr(),
                frames: 0,
                channels: (0..channels)
                    .map(|_| Channel {
                        ptr: std::ptr::null(),
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

impl<F> Drop for Converter<F>
where
    F: AudioFileBasics + AudioFileBlocks,
{
    fn drop(&mut self) {
        unsafe {
            libsamplerate_sys::src_delete(self.state);
        }
    }
}

pub struct Block {
    ptr: *const f32,
    frames: usize,
    channels: Box<[Channel]>,
}

impl super::Block for Block {
    type Channel = Channel;

    fn channel_iterators(&mut self) -> &mut [Channel] {
        for (i, channel) in self.channels.iter_mut().enumerate() {
            channel.ptr = unsafe { self.ptr.add(i) };
            channel.len = self.frames;
        }
        &mut self.channels
    }

    fn frames(&self) -> usize {
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

impl<F> AudioFileBasics for Converter<F>
where
    F: AudioFileBasics + AudioFileBlocks,
{
    fn samplerate(&self) -> usize {
        self.samplerate
    }

    fn channels(&self) -> usize {
        self.file.channels()
    }

    /// This might be one less than the actual number of frames produced by libsamplerate
    fn frames(&self) -> usize {
        (self.file.frames() as f64 * self.data.src_ratio) as usize
    }

    fn seek(&mut self, frame: usize) -> Result<(), Error> {
        // TODO: is this correct? what about rounding errors?
        self.file
            .seek((frame as f64 / self.data.src_ratio) as usize)?;
        // http://www.mega-nerd.com/SRC/api_full.html#Reset
        let result = unsafe { libsamplerate_sys::src_reset(self.state) };
        if result != 0 {
            return Err(LibSamplerateError(result).into());
        }
        self.data.input_frames = 0;
        self.data.end_of_input = 0;
        Ok(())
    }
}

impl<F> AudioFileBlocks for Converter<F>
where
    F: AudioFileBasics + AudioFileBlocks,
{
    type Block = Block;

    fn next_block(&mut self, max_frames: usize) -> Result<&mut Block, Error> {
        let channels = self.file.channels();

        // We might have to call src_process() multiple times to get some data out
        loop {
            // Get new input data (and append to already existing input data)

            let buffer = &mut self.buffer_in[(self.data.input_frames as usize * channels)..];
            if !buffer.is_empty() {
                let requested_frames = buffer.len() / channels;
                let copied_frames = self
                    .file
                    .copy_block_to_interleaved(requested_frames, buffer)?;
                if copied_frames == 0 {
                    self.data.end_of_input = 1;
                } else {
                    self.data.input_frames += copied_frames as c_long;
                }
            }

            // Call libsamplerate to get new output data

            self.data.output_frames =
                std::cmp::min(self.buffer_out.len() / channels, max_frames) as c_long;
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
