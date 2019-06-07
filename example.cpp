#include "disk_streaming.h"

int main()
{
  FileStreamer* streamer = file_streamer_new(1024, 44100);

  file_streamer_seek(streamer, 200);

  file_streamer_free(streamer);
  streamer = NULL;
}
