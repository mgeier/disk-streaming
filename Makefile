#LDFLAGS = -ldisk_streaming_ffi -Ltarget/release
LDFLAGS = -ldisk_streaming_ffi -Ltarget/debug
LDFLAGS += -ljack

CXXFLAGS = -std=c++17 -g

# export LD_LIBRARY_PATH=target/release
# export LD_LIBRARY_PATH=target/debug

example:

clean:
	$(RM) example
