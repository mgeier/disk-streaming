use std::thread;

struct FileStreamer {
    ready_queue: (),
    seek_queue: (),
    data_queue: (),
    // TODO: JoinHandle?
}

impl FileStreamer {
    fn new() -> FileStreamer {
        // TODO: create message queues

        // TODO: create data queue

        let reader_thread = thread::spawn(move || {

            // TODO: partially fill data queue

            // TODO: pass data queue to RT thread

            // TODO: thread::yield_now()

            // TODO: continue filling the queue when there is space
        });
        FileStreamer {
            ready_queue: (),
            seek_queue: (),
            data_queue: (),
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
