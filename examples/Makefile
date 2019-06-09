CPPFLAGS = -I../ffi

CXXFLAGS = -std=c++17 -g

LDFLAGS = -ldisk_streaming_ffi -L../target/release
#LDFLAGS = -ldisk_streaming_ffi -L../target/debug
LDFLAGS += -ljack

# export LD_LIBRARY_PATH=../target/release
# export LD_LIBRARY_PATH=../target/debug

example:

clean:
	$(RM) example
