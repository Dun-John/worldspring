//! Row-major 2D grids and Catmull-Rom sampling.

#[derive(Clone, Debug)]
pub struct Grid<T> {
    pub w: usize,
    pub h: usize,
    pub data: Vec<T>,
}

impl<T: Copy + Default> Grid<T> {
    pub fn new(w: usize, h: usize) -> Self {
        Self { w, h, data: vec![T::default(); w * h] }
    }

    pub fn from_vec(w: usize, h: usize, data: Vec<T>) -> Self {
        assert_eq!(data.len(), w * h);
        Self { w, h, data }
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize) -> T {
        self.data[y * self.w + x]
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, v: T) {
        let i = y * self.w + x;
        self.data[i] = v;
    }
}

/// Catmull-Rom weights for fractional position `t` in [0, 1).
#[inline]
pub fn cr_weights(t: f64) -> [f64; 4] {
    let t2 = t * t;
    let t3 = t2 * t;
    [
        0.5 * (-t3 + 2.0 * t2 - t),
        0.5 * (3.0 * t3 - 5.0 * t2 + 2.0),
        0.5 * (-3.0 * t3 + 4.0 * t2 + t),
        0.5 * (t3 - t2),
    ]
}

impl Grid<f32> {
    /// Bicubic (Catmull-Rom) sample in grid-index coordinates, clamped at the edges.
    pub fn sample_cubic(&self, x: f64, y: f64) -> f64 {
        let xf = crate::floor(x);
        let yf = crate::floor(y);
        let wx = cr_weights(x - xf);
        let wy = cr_weights(y - yf);
        let (ix, iy) = (xf as i64, yf as i64);
        let (mx, my) = (self.w as i64 - 1, self.h as i64 - 1);
        let mut acc = 0.0;
        for (j, wyj) in wy.iter().enumerate() {
            let yy = (iy - 1 + j as i64).clamp(0, my) as usize;
            let mut row = 0.0;
            for (i, wxi) in wx.iter().enumerate() {
                let xx = (ix - 1 + i as i64).clamp(0, mx) as usize;
                row += wxi * self.data[yy * self.w + xx] as f64;
            }
            acc += wyj * row;
        }
        acc
    }

    /// 2x2 box-filtered half-resolution copy (an odd trailing row/column averages with itself).
    pub fn downsample(&self) -> Grid<f32> {
        let w = self.w.div_ceil(2).max(1);
        let h = self.h.div_ceil(2).max(1);
        let mut out = Grid::new(w, h);
        for y in 0..h {
            let y0 = (2 * y).min(self.h - 1);
            let y1 = (2 * y + 1).min(self.h - 1);
            for x in 0..w {
                let x0 = (2 * x).min(self.w - 1);
                let x1 = (2 * x + 1).min(self.w - 1);
                let s = self.get(x0, y0) + self.get(x1, y0) + self.get(x0, y1) + self.get(x1, y1);
                out.set(x, y, s * 0.25);
            }
        }
        out
    }
}
