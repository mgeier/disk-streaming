use std::io::{Read, Seek};

use failure::Error;

use crate::converter::Converter;

pub mod vorbis;

pub enum AudioFile<R>
where
    R: Read + Seek,
{
    Vorbis(vorbis::File<R>),
    Resampled(Converter<R>),
}

impl<R> AudioFile<R>
where
    R: Read + Seek,
{
    pub fn new(reader: R) -> Result<AudioFile<R>, Error> {
        // TODO: try all available file types

        let file = vorbis::File::new(reader)?;
        Ok(AudioFile::Vorbis(file))
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

    /// User must ensure `buffer` is long enough, otherwise data will be lost.
    fn copy_block_to_interleaved(
        &mut self,
        frames: usize,
        buffer: &mut [f32],
    ) -> Result<usize, Error> {
        let block = self.next_block(frames)?;
        let iterators = block.channel_iterators();
        let channels = iterators.len();
        for (i, source) in iterators.iter_mut().enumerate() {
            let target = buffer[i..].iter_mut().step_by(channels);
            for (a, b) in source.zip(target) {
                *b = a
            }
        }
        Ok(block.len())
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
