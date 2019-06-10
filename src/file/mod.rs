use std::ops::IndexMut;

use failure::Error;

pub mod converter;
pub mod vorbis;
pub mod wav;

pub trait AudioFileBasics {
    fn channels(&self) -> usize;
    fn frames(&self) -> usize;
    fn samplerate(&self) -> usize;
    fn seek(&mut self, frame: usize) -> Result<(), Error>;
}

pub trait AudioFileBlocks {
    type Block: Block;

    fn next_block(&mut self, max_frames: usize) -> Result<&mut Self::Block, Error>;

    /// Panics if `buffer` is not long enough.
    fn copy_block_to_interleaved(
        &mut self,
        max_frames: usize,
        buffer: &mut [f32],
    ) -> Result<usize, Error> {
        let block = self.next_block(max_frames)?;
        let frames = block.frames();
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
            if file_block.is_empty() {
                break;
            }
            let iterators = file_block.channel_iterators();
            // TODO: check channel_map for validity?
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
            offset += file_block.frames();
        }
        // TODO: return number of frames?
        Ok(())
    }
}

pub trait Block {
    // TODO: IntoIterator?
    type Channel: Iterator<Item = f32>;
    fn channel_iterators(&mut self) -> &mut [Self::Channel];
    fn frames(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.frames() == 0
    }
}
