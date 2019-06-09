#include <iostream>
#include <vector>

#include <jack/jack.h>

// TODO: remove "extern" after new cbindgen version is released, see
//       https://github.com/eqrion/cbindgen/pull/349
extern "C" {
#include "disk_streaming.h"
}

typedef struct {
  FILE_STREAMER* streamer = nullptr;
  jack_client_t* client = nullptr;
  jack_port_t* port1 = nullptr;
  jack_port_t* port2 = nullptr;
  jack_port_t* port3 = nullptr;
  jack_port_t* port4 = nullptr;
  float** block_data = nullptr;
} userdata_t;

int sync_callback(jack_transport_state_t state, jack_position_t* pos, void* arg)
{
  auto* userdata = static_cast<userdata_t*>(arg);
  return file_streamer_seek(userdata->streamer, pos->frame);
}

int process_callback(jack_nframes_t nframes, void *arg)
{
  auto* userdata = static_cast<userdata_t*>(arg);

  jack_position_t pos;
  jack_transport_state_t state = jack_transport_query(userdata->client, &pos);

  auto* data = userdata->block_data;
  data[0] = static_cast<float*>(jack_port_get_buffer(userdata->port1, nframes));
  data[1] = static_cast<float*>(jack_port_get_buffer(userdata->port2, nframes));
  data[2] = static_cast<float*>(jack_port_get_buffer(userdata->port3, nframes));
  data[3] = static_cast<float*>(jack_port_get_buffer(userdata->port4, nframes));

  if (!file_streamer_get_data(userdata->streamer, data, state == JackTransportRolling))
  {
    std::cerr << "empty queue, stopping callback" << std::endl;
    return 1;
  }
  return 0;
}

int main()
{
  userdata_t userdata;

  jack_options_t options = JackNoStartServer;
  userdata.client = jack_client_open("file-streamer", options, nullptr);
  if (userdata.client == nullptr)
  {
    std::cerr << "Cannot create JACK client" << std::endl;
    exit(1);
  }

  auto blocksize = jack_get_buffer_size(userdata.client);
  auto samplerate = jack_get_sample_rate(userdata.client);

  userdata.streamer = file_streamer_new(blocksize, samplerate);
  if (userdata.streamer == nullptr)
  {
    std::cerr << "Cannot load files" << std::endl;
    exit(1);
  }

  // For now, 4 channels/sources are hard-coded

  std::vector<float*> storage(4);
  userdata.block_data = storage.data();

  userdata.port1 = jack_port_register(
      userdata.client, "port_1", JACK_DEFAULT_AUDIO_TYPE,
      JackPortIsOutput | JackPortIsTerminal, 0);
  if (userdata.port1 == nullptr)
  {
    std::cerr << "Cannot create JACK port" << std::endl;
    exit(1);
  }

  userdata.port2 = jack_port_register(
      userdata.client, "port_2", JACK_DEFAULT_AUDIO_TYPE,
      JackPortIsOutput | JackPortIsTerminal, 0);
  if (userdata.port2 == nullptr)
  {
    std::cerr << "Cannot create JACK port" << std::endl;
    exit(1);
  }

  userdata.port3 = jack_port_register(
      userdata.client, "port_3", JACK_DEFAULT_AUDIO_TYPE,
      JackPortIsOutput | JackPortIsTerminal, 0);
  if (userdata.port3 == nullptr)
  {
    std::cerr << "Cannot create JACK port" << std::endl;
    exit(1);
  }

  userdata.port4 = jack_port_register(
      userdata.client, "port_4", JACK_DEFAULT_AUDIO_TYPE,
      JackPortIsOutput | JackPortIsTerminal, 0);
  if (userdata.port4 == nullptr)
  {
    std::cerr << "Cannot create JACK port" << std::endl;
    exit(1);
  }

  if (jack_set_sync_callback(userdata.client, sync_callback, &userdata) != 0)
  {
    std::cerr << "Cannot set sync callback" << std::endl;
    exit(1);
  }

  if (jack_set_process_callback(userdata.client, process_callback, &userdata) != 0)
  {
    std::cerr << "Cannot set process callback" << std::endl;
    exit(1);
  }

  // TODO: stop transport? (jumping onto running transport is not supported)

  if (jack_activate(userdata.client) != 0)
  {
    std::cerr << "Cannot activate JACK client" << std::endl;
    exit(1);
  }

  std::cout << "Press <Enter> to stop" << std::endl;
  std::cin.get();

  // TODO: stop transport? (to avoid click at the end)

  jack_deactivate(userdata.client);
  jack_client_close(userdata.client);

  file_streamer_free(userdata.streamer);
  userdata.streamer = NULL;
}
