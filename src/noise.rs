use std::time::{SystemTime, UNIX_EPOCH};

/// Deterministic PRNG and 2D/3D gradient noise (Perlin/FBM) for procedural architecture.
#[derive(Clone, Copy, Debug)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    pub fn new(seed: u64) -> Self {
        Self {
            state: seed ^ 0x9E37_79B9_7F4A_7C15,
        }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform float in `[0.0, 1.0)`.
    pub fn next_f64(&mut self) -> f64 {
        let top = self.next_u64() >> 11;
        (top as f64) * (1.0 / ((1u64 << 53) as f64))
    }

    pub fn range_i32(&mut self, min_inclusive: i32, max_inclusive: i32) -> i32 {
        if max_inclusive <= min_inclusive {
            return min_inclusive;
        }
        let span = (max_inclusive - min_inclusive + 1) as u64;
        min_inclusive + (self.next_u64() % span) as i32
    }
}

/// Produces a fresh human-readable 7-digit seed using system clock nanoseconds + monotonic salt.
pub fn fresh_entropy(seq: u64, extra_salt: u64) -> u64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0u64, |d| d.as_nanos() as u64);
    let mut rng = SplitMix64::new(
        nanos ^ seq.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ extra_salt.wrapping_mul(0x517C_C1B7_2722_0A95),
    );
    (rng.next_u64() % 9_999_999).max(1)
}

/// Multi-octave deterministic gradient noise field.
#[derive(Clone, Copy, Debug)]
pub struct NoiseField {
    seed: u64,
}

impl NoiseField {
    pub const fn new(seed: u64) -> Self {
        Self { seed }
    }

    #[inline]
    fn hash3(seed: u64, x: i32, y: i32, z: i32) -> u64 {
        let mut h = seed
            ^ (x as i64 as u64).wrapping_mul(0x517C_C1B7_2722_0A95)
            ^ (y as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ (z as i64 as u64).wrapping_mul(0x94D0_49BB_1331_11EB);
        h = (h ^ (h >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        h = (h ^ (h >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        h ^ (h >> 31)
    }

    #[inline]
    fn fade(t: f64) -> f64 {
        t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
    }

    #[inline]
    fn lerp(a: f64, b: f64, t: f64) -> f64 {
        a + t * (b - a)
    }

    #[inline]
    fn grad2(hash: u64, dx: f64, dz: f64) -> f64 {
        match hash & 7 {
            0 => dx + dz,
            1 => -dx + dz,
            2 => dx - dz,
            3 => -dx - dz,
            4 => dx * 1.414_213_56,
            5 => -dx * 1.414_213_56,
            6 => dz * 1.414_213_56,
            _ => -dz * 1.414_213_56,
        }
    }

    #[inline]
    fn grad3(hash: u64, dx: f64, dy: f64, dz: f64) -> f64 {
        match hash % 12 {
            0 => dx + dy,
            1 => -dx + dy,
            2 => dx - dy,
            3 => -dx - dy,
            4 => dx + dz,
            5 => -dx + dz,
            6 => dx - dz,
            7 => -dx - dz,
            8 => dy + dz,
            9 => -dy + dz,
            10 => dy - dz,
            _ => -dy - dz,
        }
    }

    /// Smooth 2D gradient noise in approximately `[-1.0, 1.0]`.
    pub fn sample_2d(&self, x: f64, z: f64) -> f64 {
        let xi = x.floor() as i32;
        let zi = z.floor() as i32;
        let xf = x - (xi as f64);
        let zf = z - (zi as f64);
        let u = Self::fade(xf);
        let v = Self::fade(zf);

        let n00 = Self::grad2(Self::hash3(self.seed, xi, 0, zi), xf, zf);
        let n10 = Self::grad2(Self::hash3(self.seed, xi + 1, 0, zi), xf - 1.0, zf);
        let n01 = Self::grad2(Self::hash3(self.seed, xi, 0, zi + 1), xf, zf - 1.0);
        let n11 = Self::grad2(Self::hash3(self.seed, xi + 1, 0, zi + 1), xf - 1.0, zf - 1.0);

        let nx0 = Self::lerp(n00, n10, u);
        let nx1 = Self::lerp(n01, n11, u);
        Self::lerp(nx0, nx1, v).clamp(-1.0, 1.0)
    }

    /// 2D Fractal Brownian Motion normalized and contrast-expanded to `[0.0, 1.0]`.
    pub fn fbm_2d(&self, x: f64, z: f64, octaves: u8) -> f64 {
        let mut amp = 1.0;
        let mut freq = 1.0;
        let mut sum = 0.0;
        let mut norm = 0.0;
        for i in 0..octaves {
            let offset = (i as f64) * 31.7;
            sum += self.sample_2d(x * freq + offset, z * freq - offset) * amp;
            norm += amp;
            amp *= 0.5;
            freq *= 2.03;
        }
        let raw = if norm > 0.0 { sum / norm } else { 0.0 };
        ((raw * 1.65) * 0.5 + 0.5).clamp(0.0, 1.0)
    }

    /// Smooth 3D gradient noise in approximately `[-1.0, 1.0]`.
    pub fn sample_3d(&self, x: f64, y: f64, z: f64) -> f64 {
        let xi = x.floor() as i32;
        let yi = y.floor() as i32;
        let zi = z.floor() as i32;
        let xf = x - (xi as f64);
        let yf = y - (yi as f64);
        let zf = z - (zi as f64);
        let u = Self::fade(xf);
        let v = Self::fade(yf);
        let w = Self::fade(zf);

        let c000 = Self::grad3(Self::hash3(self.seed, xi, yi, zi), xf, yf, zf);
        let c100 = Self::grad3(Self::hash3(self.seed, xi + 1, yi, zi), xf - 1.0, yf, zf);
        let c010 = Self::grad3(Self::hash3(self.seed, xi, yi + 1, zi), xf, yf - 1.0, zf);
        let c110 = Self::grad3(Self::hash3(self.seed, xi + 1, yi + 1, zi), xf - 1.0, yf - 1.0, zf);
        let c001 = Self::grad3(Self::hash3(self.seed, xi, yi, zi + 1), xf, yf, zf - 1.0);
        let c101 = Self::grad3(Self::hash3(self.seed, xi + 1, yi, zi + 1), xf - 1.0, yf, zf - 1.0);
        let c011 = Self::grad3(Self::hash3(self.seed, xi, yi + 1, zi + 1), xf, yf - 1.0, zf - 1.0);
        let c111 = Self::grad3(
            Self::hash3(self.seed, xi + 1, yi + 1, zi + 1),
            xf - 1.0,
            yf - 1.0,
            zf - 1.0,
        );

        let x00 = Self::lerp(c000, c100, u);
        let x10 = Self::lerp(c010, c110, u);
        let x01 = Self::lerp(c001, c101, u);
        let x11 = Self::lerp(c011, c111, u);
        let y0 = Self::lerp(x00, x10, v);
        let y1 = Self::lerp(x01, x11, v);
        Self::lerp(y0, y1, w).clamp(-1.0, 1.0)
    }

    /// 3D FBM normalized and contrast-expanded to `[0.0, 1.0]`.
    pub fn fbm_3d(&self, x: f64, y: f64, z: f64, octaves: u8) -> f64 {
        let mut amp = 1.0;
        let mut freq = 1.0;
        let mut sum = 0.0;
        let mut norm = 0.0;
        for i in 0..octaves {
            let offset = (i as f64) * 19.3;
            sum += self.sample_3d(x * freq + offset, y * freq - offset, z * freq + offset * 0.5) * amp;
            norm += amp;
            amp *= 0.5;
            freq *= 2.03;
        }
        let raw = if norm > 0.0 { sum / norm } else { 0.0 };
        ((raw * 1.65) * 0.5 + 0.5).clamp(0.0, 1.0)
    }

    /// Deterministic hash value in `[0.0, 1.0)` for discrete per-cell variation.
    pub fn cell_hash(&self, x: i32, y: i32, z: i32) -> f64 {
        let h = Self::hash3(self.seed ^ 0xA24B_AED4_963E_E407, x, y, z) >> 11;
        (h as f64) * (1.0 / ((1u64 << 53) as f64))
    }
}
