use std::ops::{Deref, DerefMut};
use surface::{Surface, Yuv420p, Yuv422, Yuv422p};


/// interleaved -> planar
pub fn yuyv_interleave_to_yuv422p<S1, S2>(
    from: &Surface<Yuv422, u8, S1>,
    into: &mut Surface<Yuv422p, u8, S2>,
)
    where
        S1: Deref<Target=[u8]>,
        S2: Deref<Target=[u8]> + DerefMut,
{
    assert_eq!(from.width(), into.width());
    assert_eq!(from.height(), into.height());

    let mut src = from.raw_bytes().iter();
    let (yp, up, vp) = into.get_planes_mut();
    let mut yp_iter = yp.iter_mut();
    let mut up_iter = up.iter_mut();
    let mut vp_iter = vp.iter_mut();
  
    let mut subpixel_ctr = 0;
    loop {
        if let (Some(ys), Some(yd)) = (src.next(), yp_iter.next()) {
            *yd = *ys;
            subpixel_ctr += 1;
        }
        if let (Some(us), Some(ud)) = (src.next(), up_iter.next()) {
            *ud = *us;
            subpixel_ctr += 1;
        }
        if let (Some(ys), Some(yd)) = (src.next(), yp_iter.next()) {
            *yd = *ys;
            subpixel_ctr += 1;
        }
        if let (Some(vs), Some(vd)) = (src.next(), vp_iter.next()) {
            *vd = *vs;
            subpixel_ctr += 1;
        } else {
            break;
        }
    }
    assert_eq!(subpixel_ctr, from.raw_bytes().len());
}

pub fn yuv422p_from_buffer_mut(px_count: usize, buffer: &mut [u8])
-> (&mut [u8], &mut [u8], &mut [u8])
{
    // at the very least it must be even..
    debug_assert!(px_count % 2 == 0);

    let y_plane_len = px_count;
    let u_plane_len = px_count / 2;
    let v_plane_len = px_count / 2;

    let (y_plane, rest) = buffer.split_at_mut(y_plane_len);
    let (u_plane, rest) = rest.split_at_mut(u_plane_len);
    let (v_plane, _rest) = rest.split_at_mut(u_plane_len);
    (y_plane, u_plane, v_plane)
}

pub fn yuv420p_from_buffer_mut(px_count: usize, buffer: &mut [u8])
-> (&mut [u8], &mut [u8], &mut [u8])
{
    // at the very least it must be even..
    debug_assert!(px_count % 4 == 0);

    let y_plane_len = px_count;
    let u_plane_len = px_count / 4;
    let v_plane_len = px_count / 4;

    let (y_plane, rest) = buffer.split_at_mut(y_plane_len);
    let (u_plane, rest) = rest.split_at_mut(u_plane_len);
    let (v_plane, _rest) = rest.split_at_mut(u_plane_len);
    (y_plane, u_plane, v_plane)
}

pub fn downsample_yuyv_420p<S1>(from: &Surface<Yuv422, u8, S1>)
    -> Surface<Yuv420p, u8, Box<[u8]>>
    where S1: Deref<Target=[u8]>
{
    let (width, height) = (from.width() as usize, from.height() as usize);
    let pixel_count = width * height;

    let mut s422p = Surface::<Yuv422p, u8, _>::new(from.width(), from.height(),
        vec![0; pixel_count * 2].into_boxed_slice());

    yuyv_interleave_to_yuv422p(from, &mut s422p);

    let mut s420p = Surface::<Yuv420p, u8, _>::new(from.width(), from.height(),
        vec![0; pixel_count * 3 / 2].into_boxed_slice());

    {
        let (iy_pl, iu_pl, iv_pl) = s422p.get_planes_mut();
        let (oy_pl, ou_pl, ov_pl) = s420p.get_planes_mut();

        for (i, o) in iy_pl.iter().zip(oy_pl.iter_mut()) {
            *o = *i;
        }
        downsample2_uv_plane_422p_420p(width / 2, iu_pl, ou_pl);
        downsample2_uv_plane_422p_420p(width / 2, iv_pl, ov_pl);
    }

    s420p
}

pub fn downsample2_uv_plane_422p_420p<'a, 'b: 'a>(width: usize, ibuffer: &[u8], obuffer: &mut [u8])
{
    debug_assert!(obuffer.len() * 2 == ibuffer.len());
    // not sure which of these are needed...?
    debug_assert!(ibuffer.len() % 2 == 0);
    debug_assert!(ibuffer.len() % (2 * width) == 0);
    debug_assert!(obuffer.len() % 2 == 0);
    debug_assert!(obuffer.len() % (2 * width) == 0);

    for ((in0, in1), out) in RowPairIter::new(width, ibuffer.len() / width).zip(obuffer.iter_mut()) {
        *out = {
            let mut value: u16 = 0;
            value += ibuffer[in0] as u16;
            value += ibuffer[in1] as u16;
            (value >> 1) as u8
        };
    }
}

pub fn downsample_uv_plane_422p_420p<'a, 'b: 'a>(width: usize, buffer: &'a mut &'b mut [u8])
{
    debug_assert!(buffer.len() % 2 == 0);
    debug_assert!(buffer.len() % (2 * width) == 0);

    let input = (*buffer).as_mut_ptr();
    let out_len = buffer.len() / 2;

    unsafe {
        for ((in0, in1), out) in RowPairIter::new(width, buffer.len() / width).zip(0..out_len) {
            *input.offset(out as isize) = {
                let mut value: u16 = 0;
                value += *input.offset(in0 as isize) as u16;
                value += *input.offset(in1 as isize) as u16;
                (value >> 1) as u8
            };
        }
    }
    *buffer = unsafe { ::std::slice::from_raw_parts_mut(input, out_len) };
}

enum ColorSpace {
    YUYV,
    RGB,
}

struct RowPairIter {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

impl RowPairIter {
    fn new(width: usize, height: usize) -> RowPairIter {
        RowPairIter { x: 0, y: 0, width: width, height: height }
    }
}

impl Iterator for RowPairIter {
    type Item = (usize, usize);

    fn next(&mut self) -> Option<(usize, usize)> {
        if self.y >= self.height && self.x >= self.width {
            return None;
        }
        if self.x == self.width {
            self.y += 2;
            self.x = 0;
        }
        let valf = self.y * self.width + self.x;
        self.x += 1;
        Some((valf, valf + self.width))
    }
}


