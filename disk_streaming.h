#include <cstdarg>
#include <cstdint>
#include <cstdlib>
#include <new>

struct FileStreamer;

extern "C" {

void file_streamer_free(FileStreamer *ptr);

FileStreamer *file_streamer_new(size_t blocksize, size_t samplerate);

bool file_streamer_seek(FileStreamer *ptr, size_t frame);

} // extern "C"
