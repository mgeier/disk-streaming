use std::fs;

use failure::Error;

use disk_streaming::file::{converter::Converter, vorbis, AudioFileBasics, AudioFileBlocks, Block};

fn main() -> Result<(), Error> {
    //let file = fs::File::open("ukewave.ogg")?;
    let file = fs::File::open("marimba.ogg")?;
    let mut af = vorbis::File::new(file)?;

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

    println!("===========");

    buffer.clear();
    af.seek(0)?;

    let mut conv = Converter::new(af, 48_000)?;

    println!("target samplerate: {}", conv.samplerate());
    println!("target channels: {}", conv.channels());
    assert!(conv.channels() > 0);

    println!("target length:     {}", conv.frames());

    loop {
        let block = conv.next_block(1024)?;
        if block.frames() == 0 {
            break;
        }
        buffer.extend(&mut block.channel_iterators()[0]);
    }

    println!("total buffer size: {}", buffer.len());

    Ok(())
}
