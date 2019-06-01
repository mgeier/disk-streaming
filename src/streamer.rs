use std::thread;

use crossbeam::queue;

type DataConsumer = queue::spsc::Consumer<f32>;

struct FileStreamer {
    ready_consumer: queue::spsc::Consumer<DataConsumer>,
    seek_producer: queue::spsc::Producer<(usize, DataConsumer)>,
    data_consumer: Option<DataConsumer>,
    // TODO: JoinHandle?
}

impl FileStreamer {
    fn new() -> FileStreamer {
        let (ready_producer, ready_consumer) = queue::spsc::new(1);

        let (seek_producer, seek_consumer) = queue::spsc::new(1);

        let (data_producer, data_consumer) = queue::spsc::new(100_000);

        let reader_thread = thread::spawn(move || {

            data_producer;

            seek_consumer;

            // TODO: partially fill data_producer

            // TODO: pass data queue to RT thread
            ready_producer.push(data_consumer);

            // TODO: thread::yield_now()

            // TODO: continue filling the queue when there is space
        });
        FileStreamer {
            ready_consumer,
            seek_producer,
            data_consumer: None,
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

// TODO: Drop: thread.join()?

// TODO: FFI, return pointer to data? return NULL?
// TODO: use catch_unwind()? https://doc.rust-lang.org/std/panic/fn.catch_unwind.html
