#include <cstdarg>
#include <cstdint>
#include <cstdlib>
#include <new>

struct FileStreamer;

extern "C" {

void file_streamer_free(FileStreamer *ptr);

/// Return value of 0 means un-recoverable error
size_t file_streamer_get_data(FileStreamer *ptr, float *const *data);

/// Return value of 0 means un-recoverable error
size_t file_streamer_get_data_with_fade_out(FileStreamer *ptr, float *const *data);

FileStreamer *file_streamer_new(size_t blocksize, size_t samplerate);

bool file_streamer_seek(FileStreamer *ptr, size_t frame);

} // extern "C"
