use std::ops::Deref;
use surface::{Surface, Luma};


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

pub fn compose<S1, S2>(
    left: &Surface<Luma, u8, S1>,
    right: &Surface<Luma, u8, S2>,
    mode: ComposeMode,
)
    -> Surface<Luma, u8, Box<[u8]>>
    where
        S1: Deref<Target=[u8]>,
        S2: Deref<Target=[u8]>,
{
    assert_eq!(left.width(), right.width());
    assert_eq!(left.height(), right.height());

    let comp = mode.to_fn();
    let mut out = Surface::new_black(left.width(), left.height());

    for ((l, r), o) in left.raw_bytes().iter().zip(right.raw_bytes().iter()).zip(out.raw_bytes_mut().iter_mut()) {
        *o = comp(*l, *r);
    }

    out
}
