use std::fs;

use failure::Error;

use disk_streaming::file::{converter::Converter, AudioFile, Block, ProvideBlocks};

fn main() -> Result<(), Error> {
    //let file = fs::File::open("ukewave.ogg")?;
    let file = fs::File::open("marimba.ogg")?;
    let mut af = AudioFile::new(file)?;

    println!("samplerate: {}", af.samplerate());
    println!("channels: {}", af.channels());

    let mut buffer = Vec::with_capacity(af.len());
    println!("capacity: {}", buffer.capacity());

    assert!(af.channels() > 0);

    if let AudioFile::Vorbis(ref mut f) = af {
        loop {
            let block = f.next_block(1024)?;
            if block.len() == 0 {
                break;
            }
            buffer.extend(&mut block.channel_iterators()[0]);
        }
    }

    println!("total buffer size: {}", buffer.len());

    println!("===========");

    buffer.clear();
    af.seek(0)?;

    let mut conv = Converter::new(af, 48_000)?;

    println!("target samplerate: {}", conv.samplerate());
    println!("target channels: {}", conv.channels());
    assert!(conv.channels() > 0);

    println!("target length:     {}", conv.len());

    loop {
        let block = conv.next_block(1024)?;
        if block.len() == 0 {
            break;
        }
        buffer.extend(&mut block.channel_iterators()[0]);
    }

    println!("total buffer size: {}", buffer.len());

    Ok(())
}
