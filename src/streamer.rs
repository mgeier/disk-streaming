use std::io::{Read, Seek};
use std::sync::{atomic::{AtomicBool, Ordering}, Arc};
use std::thread;

use crossbeam::queue;
use failure::Error;

use crate::file::{vorbis, converter, AudioFile, DynamicAudioFile};

// TODO: loop/repeat, skip, duration ...

pub fn load_audio_file<R>(reader: R, samplerate: usize) -> Result<Box<dyn DynamicAudioFile<Box<[f32]>>>, Error>
where
    R: Read + Seek + 'static,
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
    data_consumer: queue::spsc::Consumer<Block>,
    recycling_producer: queue::spsc::Producer<Block>,
}

fn make_data_queue(
    capacity: usize,
    frames: usize,
    channels: usize,
) -> (DataProducer, DataConsumer) {
    let (data_producer, data_consumer) = queue::spsc::new(capacity);
    let (recycling_producer, recycling_consumer) = queue::spsc::new(capacity);
    for _ in 0..capacity {
        recycling_producer
            .push(Block::new(frames, channels))
            .unwrap();
    }
    (
        DataProducer {
            data_producer,
            recycling_consumer,
        },
        DataConsumer {
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
    fn sources(&mut self) -> &mut [Box<[f32]>] {
        &mut self.block.as_mut().unwrap().channels
    }

    // TODO: rename to "source()"?
    fn channel(&mut self, index: usize) -> &mut [f32] {
        &mut self.block.as_mut().unwrap().channels[index]
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
        Some(WriteBlock {
            block: Some(block),
            queue: &mut self.data_producer,
        })
    }

    fn is_full(&self) -> bool {
        self.data_producer.is_full()
    }
}

impl DataConsumer {
    fn clear(&mut self) {
        while let Ok(data) = self.data_consumer.pop() {
            self.recycling_producer.push(data).unwrap()
        }
    }
}

pub struct FileStreamer {
    ready_consumer: queue::spsc::Consumer<(usize, DataConsumer)>,
    seek_producer: queue::spsc::Producer<(usize, Option<DataConsumer>)>,
    data_consumer: Option<DataConsumer>,
    reader_thread: Option<thread::JoinHandle<()>>,
    reader_thread_keep_reading: Arc<AtomicBool>,
}

// TODO: make less public
pub struct PlaylistEntry {
    pub start: usize,
    pub end: Option<usize>,
    pub file: Box<DynamicAudioFile<Box<[f32]>> + Send>,
    pub sources: Box<[Option<usize>]>,
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
    pub fn new(playlist: Vec<PlaylistEntry>, blocksize: usize) -> FileStreamer {
        let mut playlist = playlist.into_boxed_slice();

        // TODO: provide min_buffer_duration in seconds?
        let min_blocks = 3;
        // TODO: convert max_buffer_duration into queue capacity

        let (ready_producer, ready_consumer) = queue::spsc::new(1);

        let (seek_producer, seek_consumer): (
            _,
            queue::spsc::Consumer<(usize, Option<DataConsumer>)>,
        ) = queue::spsc::new(1);

        let (mut data_producer, data_consumer) = make_data_queue(100, 512, 7);

        let reader_thread_keep_reading = Arc::new(AtomicBool::new(true));
        let keep_reading = Arc::clone(&reader_thread_keep_reading);

        // TODO: Initial "seek" message to get the whole process started?
        //seek_producer.push((0, data_consumer));

        let reader_thread = thread::spawn(move || {
            let mut queue = Some(data_consumer);
            let mut seek_frame = 0;
            let mut current_frame = 0;
            // TODO: use current_frame - seek_frame
            let mut buffered_blocks = 0;

            while keep_reading.load(Ordering::Acquire) {
                if let Ok((frame, data_consumer)) = seek_consumer.pop() {
                    //if frame != seek_frame || frame != current_frame {
                    if frame != seek_frame || data_consumer.is_some() {
                        queue = queue.or(data_consumer);

                        buffered_blocks = 0;
                        current_frame = frame;
                        seek_frame = frame;

                        let queue_block = match queue.as_mut() {
                            Some(queue) => {
                                queue.clear();
                                data_producer.write_block()
                            }
                            _ => None,
                        };

                        if let Some(mut queue_block) = queue_block {
                            let active_files = ActiveIter {
                                block_start: current_frame,
                                block_end: current_frame + blocksize,
                                inner: playlist.iter_mut(),
                            };
                            for ref mut file in active_files {
                                let offset = if file.start < seek_frame {
                                    file.file.seek(seek_frame - file.start);
                                    0
                                } else {
                                    file.file.seek(0);
                                    file.start - seek_frame
                                };
                                file.file.fill_channels(&file.sources, blocksize, offset, queue_block.sources());
                            }
                            buffered_blocks = 1;
                            current_frame += blocksize;
                        }
                    }
                }
                // TODO: simplify this condition?
                if current_frame <= seek_frame && queue.is_none() {
                    // NB: Audio thread has outdated queue, we have to wait for next "seek" message
                    continue;
                }

                if data_producer.is_full() {
                    thread::yield_now();
                    continue;
                }

                // TODO: get data from "active" files
                // TODO: on "new" files: seek(0)

                let mut block = data_producer.write_block().unwrap();
                let channel0 = block.channel(0);
                channel0[0] = 1.2;
                let channel1 = block.channel(1);
                channel1[0] = 1.2;

                // TODO: update buffered_blocks, current_frame, ...

                // TODO: get this information from the queue itself?
                // TODO: reverse the conditions?
                if buffered_blocks >= min_blocks {
                    if let Some(queue) = queue.take() {
                        ready_producer.push((seek_frame, queue));
                    }
                }
            }
        });
        FileStreamer {
            ready_consumer,
            seek_producer,
            data_consumer: None,
            reader_thread: Some(reader_thread),
            reader_thread_keep_reading,
        }
    }

    // TODO: return slice for each source?
    // TODO: possibly empty slice?
    fn get_data(&self) -> Option<f32> {
        // TODO: check if data queue is available

        // TODO: check if enough data is available

        // TODO: copy from queue or provide slice(s) into queue?

        None
    }
}

impl Drop for FileStreamer {
    fn drop(&mut self) {
        self.reader_thread_keep_reading
            .store(false, Ordering::Release);
        self.reader_thread.take().unwrap().join();
    }
}

// TODO: FFI, return pointer to data? return NULL?
// TODO: use catch_unwind()? https://doc.rust-lang.org/std/panic/fn.catch_unwind.html
