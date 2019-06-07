#LDFLAGS = -ldisk_streaming -Ltarget/release
LDFLAGS = -ldisk_streaming -Ltarget/debug
CXXFLAGS = -std=c++17

# export LD_LIBRARY_PATH=target/release
# export LD_LIBRARY_PATH=target/debug

example:
