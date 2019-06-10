use std::fs;
use std::io::BufReader;

use failure::Error;

use disk_streaming::file::{wav, AudioFileBasics, AudioFileBlocks, Block};

fn main() -> Result<(), Error> {
    let file = fs::File::open("examples/xmas.wav")?;
    //let file = fs::File::open("examples/xmas-float.wav")?;
    let reader = BufReader::new(file);
    let mut af = wav::File::new(reader)?;

    println!("samplerate: {}", af.samplerate());
    println!("channels: {}", af.channels());

    let mut buffer = Vec::with_capacity(af.frames());
    println!("capacity: {}", buffer.capacity());

    assert!(af.channels() > 0);

    loop {
        let block = af.next_block(1024)?;
        if block.frames() == 0 {
            break;
        }
        buffer.extend(&mut block.channel_iterators()[0]);
    }

    println!("total buffer size: {}", buffer.len());
    Ok(())
}
