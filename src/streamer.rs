use std::io::{Read, Seek};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::Duration;

use crossbeam::queue;
use failure::Error;

use crate::file::{converter, vorbis, AudioFileBasics, AudioFileBlocks};

/// Can be used with dynamic dispatch
pub trait AudioFile: AudioFileBasics {
    fn fill_channels(
        &mut self,
        channel_map: &[Option<usize>],
        blocksize: usize,
        offset: usize,
        channels: &mut [Box<[f32]>],
    ) -> Result<(), Error>;
}

impl<B, F> AudioFile for F
where
    B: crate::file::Block,
    F: AudioFileBlocks<Block = B> + AudioFileBasics,
{
    fn fill_channels(
        &mut self,
        channel_map: &[Option<usize>],
        blocksize: usize,
        offset: usize,
        channels: &mut [Box<[f32]>],
    ) -> Result<(), Error> {
        self.fill_channels(channel_map, blocksize, offset, channels)
    }
}

// TODO: loop/repeat, skip, duration ...

pub fn load_audio_file<R>(reader: R, samplerate: usize) -> Result<Box<dyn AudioFile + Send>, Error>
where
    R: Read + Seek + Send + 'static,
{
    let file = vorbis::File::new(reader)?;

    // TODO: try all available file types (call reader.seek(0) in between)

    if file.samplerate() == samplerate {
        Ok(Box::new(file))
    } else {
        Ok(Box::new(converter::Converter::new(file, samplerate)?))
    }
}

struct Block {
    channels: Box<[Box<[f32]>]>,
}

impl Block {
    fn new(frames: usize, channels: usize) -> Block {
        Block {
            channels: (0..channels)
                .map(|_| std::iter::repeat(0.0f32).take(frames).collect())
                .collect(),
        }
    }
}

struct DataProducer {
    data_producer: queue::spsc::Producer<Block>,
    recycling_consumer: queue::spsc::Consumer<Block>,
}

struct DataConsumer {
    blocksize: usize,
    data_consumer: queue::spsc::Consumer<Block>,
    recycling_producer: queue::spsc::Producer<Block>,
}

fn make_data_queue(
    capacity: usize,
    blocksize: usize,
    channels: usize,
) -> (DataProducer, DataConsumer) {
    let (data_producer, data_consumer) = queue::spsc::new(capacity);
    let (recycling_producer, recycling_consumer) = queue::spsc::new(capacity);
    for _ in 0..capacity {
        recycling_producer
            .push(Block::new(blocksize, channels))
            .unwrap();
    }
    (
        DataProducer {
            data_producer,
            recycling_consumer,
        },
        DataConsumer {
            blocksize,
            data_consumer,
            recycling_producer,
        },
    )
}

struct WriteBlock<'b> {
    // NB: Option in order to be able to move Block in drop()
    block: Option<Block>,
    queue: &'b mut queue::spsc::Producer<Block>,
}

impl<'b> Drop for WriteBlock<'b> {
    fn drop(&mut self) {
        if let Some(block) = self.block.take() {
            self.queue.push(block).unwrap();
        }
    }
}

impl<'b> WriteBlock<'b> {
    fn channels(&mut self) -> &mut [Box<[f32]>] {
        &mut self.block.as_mut().unwrap().channels
    }
}

impl DataProducer {
    fn write_block(&mut self) -> Option<WriteBlock> {
        let mut block = match self.recycling_consumer.pop() {
            Ok(block) => block,
            _ => return None,
        };

        // TODO: avoid filling everything with zeros?
        for channel in block.channels.iter_mut() {
            for value in channel.iter_mut() {
                *value = 0.0f32;
            }
        }
        Some(WriteBlock { block: Some(block), queue: &mut self.data_producer })
    }
}

impl DataConsumer {
    fn clear(&mut self) {
        while let Ok(data) = self.data_consumer.pop() {
            self.recycling_producer.push(data).unwrap()
        }
    }

    /// Return value of 0 means un-recoverable error
    unsafe fn write_channel_ptrs(&mut self, channels: &[*mut f32]) -> usize {
        if let Ok(block) = self.data_consumer.pop() {
            for (source, &target) in block.channels.iter().zip(channels) {
                let target = std::slice::from_raw_parts_mut(target, self.blocksize);
                target.copy_from_slice(source);
            }
            self.recycling_producer.push(block).unwrap();
            self.blocksize
        } else {
            0
        }
    }
}

pub struct FileStreamer {
    ready_consumer: queue::spsc::Consumer<(usize, DataConsumer)>,
    seek_producer: queue::spsc::Producer<(usize, DataConsumer)>,
    data_consumer: Option<DataConsumer>,
    reader_thread: Option<thread::JoinHandle<Result<(), Error>>>,
    reader_thread_keep_reading: Arc<AtomicBool>,
    channels: usize,
}

// TODO: make less public
pub struct PlaylistEntry {
    pub start: usize,
    pub end: Option<usize>,
    pub file: Box<AudioFile + Send>,
    pub channels: Box<[Option<usize>]>,
}

struct ActiveIter<'a> {
    block_start: usize,
    block_end: usize,
    inner: std::slice::IterMut<'a, PlaylistEntry>,
}

impl<'a> Iterator for ActiveIter<'a> {
    type Item = &'a mut PlaylistEntry;

    fn next(&mut self) -> Option<&'a mut PlaylistEntry> {
        while let Some(entry) = self.inner.next() {
            if entry.start < self.block_end
                && (entry.end.is_none() || self.block_start < entry.end.unwrap())
            {
                return Some(entry);
            }
        }
        None
    }
}

// TODO: different API?
// new(), add_file(), add_file, ..., start_streaming()?

impl FileStreamer {
    pub fn new(playlist: Vec<PlaylistEntry>, blocksize: usize, channels: usize) -> FileStreamer {
        let mut playlist = playlist.into_boxed_slice();

        // TODO: provide min_buffer_duration in seconds?
        let min_frames = 4096;
        // TODO: convert max_buffer_duration into queue capacity
        let capacity = 100;

        let (ready_producer, ready_consumer) = queue::spsc::new(1);
        let (seek_producer, seek_consumer) = queue::spsc::new::<(usize, DataConsumer)>(1);
        let (mut data_producer, data_consumer) = make_data_queue(capacity, blocksize, channels);

        let reader_thread_keep_reading = Arc::new(AtomicBool::new(true));
        let keep_reading = Arc::clone(&reader_thread_keep_reading);

        let reader_thread = thread::spawn(move || -> Result<(), Error> {
            let mut data_consumer = Some(data_consumer);
            let mut current_frame = 0;
            let mut seek_frame = 0;

            while keep_reading.load(Ordering::Acquire) {
                if let Ok((frame, mut queue)) = seek_consumer.pop() {
                    queue.clear();
                    data_consumer = Some(queue);
                    current_frame = frame;
                    seek_frame = frame;
                    println!("received seek {}", seek_frame);
                }
                let mut block = match data_producer.write_block() {
                    Some(block) => block,
                    None => {
                        // TODO: configurable sleep time?
                        thread::sleep(Duration::from_micros(1_000));
                        continue;
                    }
                };
                println!("preparing to write block");
                let active_files = ActiveIter {
                    block_start: current_frame,
                    block_end: current_frame + blocksize,
                    inner: playlist.iter_mut(),
                };
                for ref mut file in active_files {
                    let offset = if file.start < current_frame {
                        if current_frame == seek_frame {
                            file.file.seek(current_frame - file.start)?;
                        }
                        0
                    } else {
                        file.file.seek(0)?;
                        file.start - current_frame
                    };
                    file.file.fill_channels(
                        &file.channels,
                        blocksize,
                        offset,
                        block.channels(),
                        )?;
                }
                current_frame += blocksize;

                // Make sure the block is queued before data_consumer is sent
                drop(block);

                // TODO: get this information from the queue itself?
                if current_frame - seek_frame >= min_frames {
                    if let Some(data_consumer) = data_consumer.take() {
                        // There is only one data queue, push() will always succeed
                        ready_producer.push((seek_frame, data_consumer)).unwrap();
                    }
                }
            }
            Ok(())
        });
        FileStreamer {
            ready_consumer,
            seek_producer,
            data_consumer: None,
            reader_thread: Some(reader_thread),
            reader_thread_keep_reading,
            channels,
        }
    }

    /// Return value of 0 means un-recoverable error
    pub unsafe fn get_data(&mut self, target: &[*mut f32]) -> usize {
        // TODO: check if disk thread is still running?

        if let Some(ref mut queue) = self.data_consumer {
            println!("getting data");
            queue.write_channel_ptrs(target)
        } else {
            println!("no data_consumer");
            0
        }
    }

    pub fn channels(&self) -> usize {
        self.channels
    }

    pub fn seek(&mut self, frame: usize) -> bool {
        // TODO: check if disk thread is still running?

        if self.data_consumer.is_none() {
            // NB: There can never be more than one message
            if let Ok((ready_frame, queue)) = self.ready_consumer.pop() {
                self.data_consumer = Some(queue);
                if ready_frame == frame {
                    return true;
                }
            }
        }
        if let Some(queue) = self.data_consumer.take() {
            self.seek_producer.push((frame, queue)).unwrap();
        }
        false
    }
}

impl Drop for FileStreamer {
    fn drop(&mut self) {
        self.reader_thread_keep_reading
            .store(false, Ordering::Release);
        // TODO: handle error from closure? log errors?
        self.reader_thread.take().unwrap().join();
    }
}

// TODO: FFI, return pointer to data? return NULL?
// TODO: use catch_unwind()? https://doc.rust-lang.org/std/panic/fn.catch_unwind.html
