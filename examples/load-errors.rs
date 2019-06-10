use disk_streaming::streamer::load_audio_file;


fn main() {
    //match load_audio_file("examples/marimba.ogg", 48_000) {
    match load_audio_file("examples/load-errors.rs", 48_000) {
        Ok(_) => println!("success"),
        //Err(e) => eprintln!("error: {}", e),
        Err(e) => eprintln!("error: {:?}", e),
    }
}
