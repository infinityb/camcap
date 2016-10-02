use std::borrow::Cow;

use ::surface::planar::{PlanarSurface, Luma, Yuv420p};

use super::conversions::{
    yuyv_interleave_to_yuv422p,
    yuv420p_from_buffer_mut,
    downsample_yuyv_420p,
};

#[derive(Clone)]
pub struct Yuv420pSurface {
    surface: PlanarSurface<Yuv420p, u8>,
}

impl Yuv420pSurface {
    pub fn new_black(width: u32, height: u32) -> Yuv420pSurface {
        Yuv420pSurface {
            surface: PlanarSurface::new_black(width, height)
        }
    }

    pub fn from_yuyv_buf(width: u32, height: u32, data: &[u8]) -> Yuv420pSurface {
        // FIXME: copy+alloc!
        let buf = downsample_yuyv_420p(width, height, data);
        Yuv420pSurface {
            surface: PlanarSurface::new(width, height, &buf)
        }
    }

    pub fn width(&self) -> u32 {
        self.surface.width()
    }

    pub fn height(&self) -> u32 {
        self.surface.height()
    }


    pub fn luma_bytes(&self) -> &[u8] {
        let (width, height) = (self.width() as usize, self.height() as usize);

        let data = self.surface.raw_bytes();
        &data[..width * height]
    }

    pub fn u_bytes(&self) -> &[u8] {
        let (width, height) = (self.width() as usize, self.height() as usize);
        let luma_len = width * height;
        let chroma_len = width * height / 4;

        let data = self.surface.raw_bytes();
        &data[luma_len..][..chroma_len]
    }

    pub fn v_bytes(&self) -> &[u8] {
        let (width, height) = (self.width() as usize, self.height() as usize);
        let luma_len = width * height;
        let chroma_len = width * height / 4;

        let data = self.surface.raw_bytes();
        &data[luma_len..][chroma_len..][..chroma_len]
    }

    pub fn luma_surf(&self) -> LumaSurface {
        LumaSurface {
            surface: self.surface.extract_luma()
        }
    }

    pub fn raw_bytes(&self) -> &[u8] {
        self.surface.raw_bytes()
    }

    pub fn raw_bytes_mut(&mut self) -> &mut [u8] {
        self.surface.raw_bytes_mut()
    }
}

#[derive(Clone)]
pub struct LumaSurface {
    surface: PlanarSurface<Luma, u8>,
}

impl LumaSurface {
    pub fn new_black(width: u32, height: u32) -> LumaSurface
    {
        LumaSurface {
            surface: PlanarSurface::new_black(width, height),
        }
    }

    pub fn new(width: u32, height: u32, buf: &[u8]) -> LumaSurface
    {
        LumaSurface {
            surface: PlanarSurface::new(width, height, buf),
        }
    }

    pub fn width(&self) -> u32
    {
        self.surface.width()
    }

    pub fn height(&self) -> u32
    {
        self.surface.height()
    }

    pub fn run_kernel_3x3(&self, kernel: fn(&[u8; 9]) -> u8) -> LumaSurface
    {
        let surface = self.surface.run_luma8_kernel_3x3(kernel);
        LumaSurface {
            surface: surface,
        }
    }

    pub fn raw_bytes(&self) -> &[u8]
    {
        self.surface.raw_bytes()
    }
}

pub fn l8_sobel_3x3(pixels: &[u8; 9]) -> u8 {
    let mut acc_x = 0;
    let mut acc_y = 0;

    // acc_x
    acc_x -= 1 * pixels[0 + 3 * 0] as i32;      // (x=0, y=0)
    acc_x += 1 * pixels[2 + 3 * 0] as i32;      // (x=2, y=0)

    acc_x -= 2 * pixels[0 + 3 * 1] as i32;      // (x=0, y=1)
    acc_x += 2 * pixels[2 + 3 * 1] as i32;      // (x=2, y=1)

    acc_x -= 1 * pixels[0 + 3 * 2] as i32;      // (x=0, y=2)
    acc_x += 1 * pixels[2 + 3 * 2] as i32;      // (x=2, y=2)

    // acc_y
    acc_y -= 1 * pixels[0 + 3 * 0] as i32;      // (x=0, y=0)
    acc_y -= 2 * pixels[1 + 3 * 0] as i32;      // (x=1, y=0)
    acc_y -= 1 * pixels[2 + 3 * 0] as i32;      // (x=2, y=0)

    acc_y += 1 * pixels[0 + 3 * 2] as i32;      // (x=0, y=2)
    acc_y += 2 * pixels[1 + 3 * 2] as i32;      // (x=1, y=2)
    acc_y += 1 * pixels[2 + 3 * 2] as i32;      // (x=2, y=2)


    let acc_s = ((acc_y * acc_y + acc_x * acc_x)) as f32;
    clamp(acc_s.sqrt().round() as i32, 0x00, 0xFF) as u8
}

pub fn l8_average_3x3(pixels: &[u8; 9]) -> u8 {
    let mut acc = 0;

    for px in pixels.iter() {
        acc += *px as i16;
    }

    (acc / 9) as u8
}


#[inline(always)]
fn clamp<T: Ord>(val: T, minv: T, maxv: T) -> T {
    use std::cmp::{min, max};
    max(min(val, maxv), minv)
}

pub enum ComposeMode {
    AbsoluteDiff,
    Average,
    AverageLeftWeight,
}

impl ComposeMode {
    pub fn to_fn(&self) -> fn(u8, u8) -> u8 {
        match *self {
            ComposeMode::AbsoluteDiff => compose_absolute_diff,
            ComposeMode::Average => compose_average,
            ComposeMode::AverageLeftWeight => compose_average_left_weight,
        }
    }
}

fn compose_absolute_diff(left: u8, right: u8) -> u8 {
    let (left, right) = (left as i16, right as i16);
    (left - right).abs() as u8
}

fn compose_average(left: u8, right: u8) -> u8 {
    let (left, right) = (left as i16, right as i16);
    ((left + right) / 2) as u8
}

fn compose_average_left_weight(left: u8, right: u8) -> u8 {
    let (left, right) = (left as i16, right as i16);
    ((2 * left + right) / 3) as u8
}

pub fn compose(
    left: &Surface<Luma, u8, S1>,
    right: &Surface<Luma, u8, S2>,
    mode: ComposeMode,
) -> Surface<Luma, u8, Box<[u8]>> {
    assert_eq!(left.width(), right.width());
    assert_eq!(left.height(), right.height());

    let comp = mode.to_fn();
    let (left, right) = (&left.surface, &right.surface);

    let mut out = Surface::new_black(left.width(), left.height());

    for ((l, r), o) in left.raw_bytes().iter().zip(right.raw_bytes().iter()).zip(out.raw_bytes_mut().iter_mut()) {
        *o = comp(*l, *r);
    }

    out
}
