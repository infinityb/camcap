extern crate webp_sys;
extern crate rscam;
extern crate time;
extern crate byteorder;
extern crate fallocate;
extern crate surface;

use std::env;
use std::fs;
use std::io::{self, Write};
use std::sync::Arc;
use std::sync::mpsc::channel;
use std::sync::atomic::{AtomicIsize, Ordering};
use byteorder::{WriteBytesExt, BigEndian};
use std::collections::VecDeque;
use std::sync::mpsc::sync_channel;
use std::thread;

use surface::{Surface, Luma, Yuv420p, Yuv422p, Yuv422};
use surface::kernels::{Luma8Sobel3x3, Luma8Average3x3};

mod webp;
mod compose;
mod conversions;
mod punchcat;

use self::punchcat::PunchCat;
use self::conversions::{
    yuyv_interleave_to_yuv422p,
    downsample_yuyv_420p,
};
use self::compose::{compose, ComposeMode};

const PREAMBLE: &'static [u8] = &[0x98, 0x56, 0xcb, 0x6b, 0x56, 0xf8, 0xc8, 0x15];

fn main() {
    const WIDTH: u32 = 1280;
    const HEIGHT: u32 = 960;
    const LIT_HISTORY_MAX: usize = 3;

    let ctr = Arc::new(AtomicIsize::new(0));

    let video_dev = env::args().nth(1).unwrap();
    let prefix = env::args().nth(2).unwrap();

    let mut camera = rscam::new(&video_dev).unwrap();

    camera.start(&rscam::Config {
        interval: (1, 5),      // 5 fps.
        resolution: (WIDTH, HEIGHT),
        format: b"YUYV",
        ..Default::default()
    }).expect("camera open fail");

    let now = time::get_time();


    let filename_fs = format!("{}_{}.{:09}_fs.fwebp", prefix, now.sec, now.nsec);
    println!("writing fullsize to {}", filename_fs);

    let filename_yuv = format!("{}_{}.{:09}.yuv422p", prefix, now.sec, now.nsec);
    println!("writing raw to {} | interval = {}", filename_yuv, 2 * WIDTH * HEIGHT);

    let filename_edge = format!("{}_{}.{:09}.edge.yuv420p", prefix, now.sec, now.nsec);
    println!("writing raw to {} | interval = {}", filename_edge, 3 * WIDTH * HEIGHT / 2);

    let mut mctx = MotionContext::new(WIDTH, HEIGHT);
    let mut frameout_fs = fs::File::create(&filename_fs).unwrap();
    let mut frameout_edge = PunchCat::new(27, 26, fs::File::create(&filename_edge).unwrap());
    let mut frameout_yuv = PunchCat::new(27, 26, fs::File::create(&filename_yuv).unwrap());

    let (tx, rx) = sync_channel(10);
    let camera_thread = thread::spawn(move || {
        for i in 0_u64.. {
            let frame_when = time::get_time();

            let frame_data = camera.capture().unwrap().to_vec().into_boxed_slice();
            let surf = Surface::<Yuv422, u8, _>::new(WIDTH, HEIGHT, frame_data);
            tx.send((i, frame_when, surf)).unwrap();
        }
    });

    let mut out_surf = Surface::<Yuv422p, u8, _>::new_black(WIDTH, HEIGHT);
    for (i, frame_when, surf) in rx {
        yuyv_interleave_to_yuv422p(&surf, &mut out_surf);
        frameout_yuv.write_all(out_surf.raw_bytes()).unwrap();

        let surf_ds = downsample_yuyv_420p(&surf);
        if let Some(emit_surf) = mctx.push_pop(surf_ds) {
            let tcode_st = time::get_time();
            let webp = webp::reencode(&emit_surf);
            println!("transcode time: {}", time::get_time() - tcode_st);

            frameout_fs.write_all(PREAMBLE).unwrap();
            frameout_fs.write_i64::<BigEndian>(frame_when.sec).unwrap();
            frameout_fs.write_i32::<BigEndian>(frame_when.nsec).unwrap();
            frameout_fs.write_u32::<BigEndian>(webp.len() as u32).unwrap();
            frameout_fs.write_all(&webp[..]).unwrap();

            println!("emit F#{:010} @{}.{:09} len={}", i, frame_when.sec, frame_when.nsec, webp.len());
        }

        write_lumasurface_yuv420p(&mut frameout_edge, &mctx.last_edge).unwrap();
    }

    camera_thread.join().unwrap();
}

// cargo run --release | mpv /dev/stdin --demuxer=rawvideo --demuxer-rawvideo=w=1280:h=960
// ffmpeg -f rawvideo -video_size 1280x960 -framerate 5 /dev/stdin foo.webm


struct MotionContext {
    denoise_avg: Surface<Luma, u8, Box<[u8]>>,
    last_edge: Surface<Luma, u8, Box<[u8]>>,
    recents: VecDeque<(usize, Surface<Yuv420p, u8, Box<[u8]>>)>,
    emit_ctr: usize,
    back_window: usize,
}

impl MotionContext {
    pub fn new(width: u32, height: u32) -> MotionContext {
        MotionContext {
            denoise_avg: Surface::new_black(width, height),
            last_edge: Surface::new_black(width, height),
            recents: VecDeque::new(),
            emit_ctr: 0,
            back_window: 10,
        }
    }

    //
    pub fn push_pop(&mut self, frame: Surface<Yuv420p, u8, Box<[u8]>>)
        -> Option<Surface<Yuv420p, u8, Box<[u8]>>>
    {
        let edge = {
            let (y_p, _, _) = frame.get_planes();
            let frame_luma = Surface::<Luma, u8, _>::new(frame.width(), frame.height(), y_p);

            let mut tmp = Surface::<Luma, u8, _>::new_black(frame.width(), frame.height());
            frame_luma.run_kernel_3x3(&Luma8Average3x3, &mut tmp);

            let mut edge = Surface::<Luma, u8, _>::new_black(frame.width(), frame.height());
            tmp.run_kernel_3x3(&Luma8Sobel3x3, &mut edge);

            edge
        };

        self.last_edge = compose(&self.denoise_avg, &edge, ComposeMode::AbsoluteDiff);
        self.denoise_avg = edge;

        let mut lit_pixels = 0;
        let mut max_val = 0;
        for px in self.last_edge.raw_bytes() {
            max_val = ::std::cmp::max(max_val, *px);
            if 0x60 < *px {
                lit_pixels += 1;
            }
        }

        self.recents.push_back((lit_pixels, frame));

        let mut emit_frame = None;
        if self.recents.len() > self.back_window {
            let (_lit_px, surf) = self.recents.pop_front().unwrap();
            emit_frame = Some(surf);
        }

        if self.recents.len() * 100 < self.recents.iter().map(|&(v, _)| v).sum() {
            self.emit_ctr = 12;
        }
        if self.emit_ctr > 0 {
            self.emit_ctr -= 1;
            return emit_frame;
        }
        return None;
    }
}

fn write_lumasurface_yuv420p<W: Write>(wri: &mut W, surf: &Surface<Luma, u8, Box<[u8]>>) -> io::Result<()> {
    let (width, height) = (surf.width() as usize, surf.height() as usize);

    let chroma_hack = vec![0x80; width * height / 4];   
    try!(wri.write_all(surf.raw_bytes()));
    try!(wri.write_all(&chroma_hack[..]));
    try!(wri.write_all(&chroma_hack[..]));

    Ok(())
}
