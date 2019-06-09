#ifndef DISK_STREAMING_H
#define DISK_STREAMING_H

/* Generated with cbindgen:0.8.7 */

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct FILE_STREAMER FILE_STREAMER;

void file_streamer_free(FILE_STREAMER *ptr);

/**
 * Return value of `false` means un-recoverable error
 */
bool file_streamer_get_data(FILE_STREAMER *ptr, float *const *data, bool rolling);

FILE_STREAMER *file_streamer_new(size_t blocksize, size_t samplerate);

bool file_streamer_seek(FILE_STREAMER *ptr, size_t frame);

#endif /* DISK_STREAMING_H */
