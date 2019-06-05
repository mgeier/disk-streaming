use std::io::{Read, Seek};
use std::ops::IndexMut;

use failure::Error;

pub mod converter;
pub mod vorbis;

pub enum AudioFile<R>
where
    R: Read + Seek,
{
    Vorbis(vorbis::File<R>),
    Resampled(converter::Converter<R>),
}

impl<R> AudioFile<R>
where
    R: Read + Seek,
{
    // TODO: loop/repeat, skip, duration ... use builder pattern?

    pub fn new(reader: R) -> Result<AudioFile<R>, Error> {
        // TODO: try all available file types (call reader.seek(0) in between)

        let file = AudioFile::Vorbis(vorbis::File::new(reader)?);

        Ok(file)
    }

    pub fn with_samplerate(reader: R, samplerate: usize) -> Result<AudioFile<R>, Error> {
        let file = AudioFile::new(reader)?;
        if file.samplerate() == samplerate {
            Ok(file)
        } else {
            Ok(AudioFile::Resampled(converter::Converter::new(
                file, samplerate,
            )?))
        }
    }

    pub fn samplerate(&self) -> usize {
        use AudioFile::*;
        match self {
            Vorbis(f) => f.samplerate(),
            Resampled(f) => f.samplerate(),
        }
    }

    pub fn channels(&self) -> usize {
        use AudioFile::*;
        match self {
            Vorbis(f) => f.channels(),
            Resampled(f) => f.channels(),
        }
    }

    pub fn len(&self) -> usize {
        use AudioFile::*;
        match self {
            Vorbis(f) => f.len(),
            Resampled(f) => f.len(),
        }
    }

    pub fn seek(&mut self, frame: usize) -> Result<(), Error> {
        use AudioFile::*;
        match self {
            Vorbis(f) => f.seek(frame),
            Resampled(f) => f.seek(frame),
        }
    }
}

pub trait ProvideBlocks {
    type Block: Block;
    fn next_block(&mut self, frames: usize) -> Result<&mut Self::Block, Error>;

    /// Panics if `buffer` is not long enough.
    fn copy_block_to_interleaved(
        &mut self,
        frames: usize,
        buffer: &mut [f32],
    ) -> Result<usize, Error> {
        let block = self.next_block(frames)?;
        let frames = block.len();
        let iterators = block.channel_iterators();
        let channels = iterators.len();
        for frame in 0..frames {
            for channel in 0..channels {
                buffer[frame * channels + channel] = iterators[channel].next().unwrap();
            }
        }
        // TODO: benchmark alternative implementation
        /*
        for (i, source) in iterators.iter_mut().enumerate() {
            let target = buffer[i..].iter_mut().step_by(channels);
            for (a, b) in source.zip(target) {
                *b = a
            }
        }
        */
        Ok(frames)
    }

    fn fill_channels<D>(
        &mut self,
        channel_map: &[Option<usize>],
        blocksize: usize,
        offset: usize,
        channels: &mut [D],
    ) -> Result<(), Error>
    where
        D: std::ops::DerefMut<Target = [f32]>,
    {
        let mut offset = offset;
        while offset < blocksize {
            let file_block = self.next_block(blocksize - offset)?;
            let iterators = file_block.channel_iterators();
            for (i, &channel) in channel_map.iter().enumerate() {
                if let Some(channel) = channel {
                    // TODO: use iterators[i]?
                    for (a, b) in
                        IndexMut::index_mut(iterators, i).zip(&mut channels[channel][offset..])
                    {
                        *b = a;
                    }
                }
            }
            offset += file_block.len();
        }
        Ok(())
    }
}

pub trait Block {
    type Channel: Iterator<Item = f32>;
    fn channel_iterators(&mut self) -> &mut [Self::Channel];
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
