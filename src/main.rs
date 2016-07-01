extern crate rscam;
extern crate time;
extern crate byteorder;
// extern crate image;

use std::env;
use std::fs;
use std::io::{self, Write};
use std::sync::Arc;
use std::sync::mpsc::channel;
use std::sync::atomic::{AtomicIsize, Ordering};
use byteorder::{WriteBytesExt, BigEndian};
// use image::{
//     ImageResult,
//     ImageFormat,
//     FilterType,
// };

const PREAMBLE: &'static [u8] = &[0x98, 0x56, 0xcb, 0x6b, 0x56, 0xf8, 0xc8, 0x15];

fn main() {
    let ctr = Arc::new(AtomicIsize::new(0));

    let video_dev = env::args().nth(1).unwrap();
    let prefix = env::args().nth(2).unwrap();

    let mut camera = rscam::new(&video_dev).unwrap();

    camera.start(&rscam::Config {
        interval: (1, 5),      // 30 fps.
        resolution: (1280, 960),
        format: b"MJPG",
        ..Default::default()
    }).unwrap();

    let now = time::get_time();
    let filename_fs = format!("{}_{}.{:09}_fs.fmjpg", prefix, now.sec, now.nsec);
    let filename_sc = format!("{}_{}.{:09}_sc.fmjpg", prefix, now.sec, now.nsec);

    println!("writing fullsize to {}", filename_fs);
    println!("writing scaled to {}", filename_sc);

    let mut frameout_fs = fs::File::create(&filename_fs).unwrap();
    // let mut frameout_sc = fs::File::create(&filename_sc).unwrap();

    // let (tx, rx) = channel::<(time::Timespec, rscam::Frame)>();
    // let tcode_ctr = ctr.clone();
    // let transcode = ::std::thread::spawn(move || {
    //     for (frame_when, frame) in rx {
    //         tcode_ctr.fetch_sub(1, Ordering::SeqCst);
    //         if let Ok(smaller_frame) = rescale(&frame[..]) {

    //             frameout_sc.write_all(PREAMBLE).unwrap();
    //             frameout_sc.write_i64::<BigEndian>(frame_when.sec).unwrap();
    //             frameout_sc.write_i32::<BigEndian>(frame_when.nsec).unwrap();
    //             frameout_sc.write_u32::<BigEndian>(smaller_frame.len() as u32).unwrap();
    //             frameout_sc.write_all(&smaller_frame[..]).unwrap();
    //             println!("emit compressed frame");
    //         }
    //     }
    // });

    for i in 0_u64.. {
        let frame_when = time::get_time();
        let frame = camera.capture().unwrap();
        let frame_len = frame.len();

        frameout_fs.write_all(PREAMBLE).unwrap();
        frameout_fs.write_i64::<BigEndian>(frame_when.sec).unwrap();
        frameout_fs.write_i32::<BigEndian>(frame_when.nsec).unwrap();
        frameout_fs.write_u32::<BigEndian>(frame.len() as u32).unwrap();
        frameout_fs.write_all(&frame[..]).unwrap();

        // let ctr_val = ctr.fetch_add(1, Ordering::SeqCst);
        // if 5 < ctr_val {
        //     println!("Transcode is falling behind.");
        // }
        // if let Err(err) = tx.send((frame_when, frame)) {
        //     println!("Transcode TX failure: {}", err);
        // }
        println!("emit F#{:010} @{}.{:09} len={}", i, frame_when.sec, frame_when.nsec, frame_len);
    }

    // drop(transcode);
}


// fn rescale(buf: &[u8]) -> ImageResult<Vec<u8>> {
//     let image = try!(image::load_from_memory(buf));
//     let mut out = io::Cursor::new(Vec::with_capacity(512 * 1024));
//     try!(image.resize(640, 480, FilterType::Lanczos3).save(&mut out, ImageFormat::JPEG));
//     Ok(out.into_inner())
// }
