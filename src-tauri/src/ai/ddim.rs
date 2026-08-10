//! The DDIM scheduler — the recipe that turns a pile of noise into a picture.
//!
//! A diffusion model does not draw. It looks at a noisy image and says "here is
//! the noise I think is in it". Removing all of that at once gives mush; the
//! scheduler is what decides how much to remove at each of the twenty-odd steps,
//! and it is pure arithmetic — no model, no GPU.
//!
//! Which is why it lives in its own file with tests. Getting a coefficient wrong
//! here does not crash and does not warn: the loop runs, an image comes out, and
//! it is grey soup. Everything below is checked against the numbers in the
//! model's own `scheduler_config.json`:
//!
//! ```json
//! { "beta_start": 0.00085, "beta_end": 0.012, "beta_schedule": "scaled_linear",
//!   "num_train_timesteps": 1000, "prediction_type": "epsilon",
//!   "steps_offset": 1, "timestep_spacing": "leading",
//!   "set_alpha_to_one": false, "clip_sample": false }
//! ```

/// How many steps the model was trained over. The inference loop uses far fewer,
/// spaced out across this range.
const TRAIN_STEPS: usize = 1000;
const BETA_START: f64 = 0.00085;
const BETA_END: f64 = 0.012;
/// `steps_offset: 1` in the config. It shifts every timestep up by one, which is
/// a quirk of the original Stable Diffusion release that the weights now expect.
const STEPS_OFFSET: usize = 1;

pub struct Ddim {
    /// ᾱ for each of the 1000 training steps: how much of the original image is
    /// left at that noise level.
    alphas_cumprod: Vec<f64>,
    /// The timesteps this run will visit, from noisiest to cleanest.
    timesteps: Vec<usize>,
    /// Gap between visited timesteps, needed to find the previous ᾱ.
    stride: usize,
}

impl Ddim {
    /// Builds the schedule for `steps` denoising passes.
    pub fn new(steps: usize) -> Self {
        let steps = steps.clamp(1, TRAIN_STEPS);

        // "scaled_linear": the betas are linear in their *square roots*, not in
        // themselves. Using a plain linear ramp is the classic silent mistake —
        // it produces washed-out images rather than an error.
        let start = BETA_START.sqrt();
        let end = BETA_END.sqrt();
        let mut alphas_cumprod = Vec::with_capacity(TRAIN_STEPS);
        let mut running = 1.0;
        for i in 0..TRAIN_STEPS {
            let t = i as f64 / (TRAIN_STEPS - 1) as f64;
            let beta = (start + (end - start) * t).powi(2);
            running *= 1.0 - beta;
            alphas_cumprod.push(running);
        }

        // "leading" spacing: evenly spaced from 0, then shifted by the offset and
        // walked backwards.
        let stride = TRAIN_STEPS / steps;
        let timesteps: Vec<usize> = (0..steps).map(|i| i * stride + STEPS_OFFSET).rev().collect();

        Self {
            alphas_cumprod,
            timesteps,
            stride,
        }
    }

    /// The timesteps to run, noisiest first.
    pub fn timesteps(&self) -> &[usize] {
        &self.timesteps
    }

    fn alpha(&self, t: usize) -> f64 {
        self.alphas_cumprod[t.min(TRAIN_STEPS - 1)]
    }

    /// ᾱ of the step before `t`.
    ///
    /// Past the start of the schedule there is no previous step. `set_alpha_to_one`
    /// is false in this model's config, so the first entry stands in — using 1.0
    /// instead, which some implementations do, brightens every result.
    fn alpha_prev(&self, t: usize) -> f64 {
        match t.checked_sub(self.stride) {
            Some(previous) => self.alpha(previous),
            None => self.alphas_cumprod[0],
        }
    }

    /// Adds `t`'s worth of noise to a clean sample — the forward process.
    ///
    /// Used to put the untouched part of the photo back at each step: it has to
    /// re-enter the loop carrying exactly the noise the model expects to see at
    /// that point, or the seam between kept and generated shows.
    pub fn add_noise(&self, clean: &[f32], noise: &[f32], t: usize) -> Vec<f32> {
        let alpha = self.alpha(t);
        let (keep, add) = (alpha.sqrt() as f32, (1.0 - alpha).sqrt() as f32);
        clean
            .iter()
            .zip(noise.iter())
            .map(|(sample, noise)| keep * sample + add * noise)
            .collect()
    }

    /// One denoising step: from the sample at `t` to the sample at the step before.
    ///
    /// `predicted_noise` is what the UNet just returned. `prediction_type` is
    /// "epsilon", so the model predicts the noise itself rather than the image.
    pub fn step(&self, sample: &[f32], predicted_noise: &[f32], t: usize) -> Vec<f32> {
        let alpha = self.alpha(t);
        let alpha_prev = self.alpha_prev(t);

        let (sqrt_alpha, sqrt_one_minus) = (alpha.sqrt() as f32, (1.0 - alpha).sqrt() as f32);
        let (sqrt_alpha_prev, sqrt_one_minus_prev) =
            (alpha_prev.sqrt() as f32, (1.0 - alpha_prev).sqrt() as f32);

        sample
            .iter()
            .zip(predicted_noise.iter())
            .map(|(sample, noise)| {
                // Undo the noise to guess the clean image, then re-noise it to
                // the *previous*, gentler level. `clip_sample` is false, so the
                // guess is left unclamped.
                let original = (sample - sqrt_one_minus * noise) / sqrt_alpha;
                sqrt_alpha_prev * original + sqrt_one_minus_prev * noise
            })
            .collect()
    }

    /// The starting noise level, for scaling the initial latents.
    pub fn initial_noise_sigma(&self) -> f32 {
        // DDIM works on unscaled latents, unlike the Euler family. Kept explicit
        // so swapping in another scheduler does not silently drop the scaling.
        1.0
    }
}

/// Deterministic noise, so the same photo and prompt give the same picture twice.
///
/// A seeded generator rather than the system one: "run it again and get the same
/// thing" is what makes a result something you can keep, and an unseeded pipeline
/// cannot offer it.
pub struct Noise {
    state: u64,
}

impl Noise {
    pub fn seeded(seed: u64) -> Self {
        // Any odd constant works to avoid the zero state.
        Self {
            state: seed ^ 0x9E37_79B9_7F4A_7C15,
        }
    }

    /// Uniform in 0..1, from SplitMix64.
    fn next_uniform(&mut self) -> f64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        // 53 bits of mantissa, shifted into 0..1, never exactly 0.
        ((z >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }

    /// A standard normal sample, via Box–Muller.
    ///
    /// The models were trained on Gaussian noise. Uniform noise would run
    /// perfectly well and produce flat, washed-out images.
    pub fn normal(&mut self) -> f32 {
        let u1 = self.next_uniform();
        let u2 = self.next_uniform();
        ((-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()) as f32
    }

    pub fn vector(&mut self, len: usize) -> Vec<f32> {
        (0..len).map(|_| self.normal()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_noise_schedule_matches_the_published_numbers() {
        let ddim = Ddim::new(25);
        // Computed from the diffusers formula against this model's config, not
        // recalled: `betas = linspace(√0.00085, √0.012, 1000)²`, then cumulative
        // product of `1 - beta`. Three points pin the whole curve — the ends and
        // the middle, which no off-by-one or wrong ramp can all satisfy at once.
        assert!((ddim.alphas_cumprod[0] - 0.99915).abs() < 1e-5);
        assert!((ddim.alphas_cumprod[499] - 0.2776696).abs() < 1e-5);
        assert!((ddim.alphas_cumprod[999] - 0.0046601).abs() < 1e-6);
        // Monotonically decreasing: each step keeps less of the original.
        assert!(ddim
            .alphas_cumprod
            .windows(2)
            .all(|pair| pair[1] < pair[0]));
    }

    #[test]
    fn a_linear_ramp_would_have_given_different_numbers() {
        // Guards the "scaled_linear" reading: betas are linear in their square
        // roots. A plain linear ramp is the classic silent mistake, and it lands
        // somewhere clearly different.
        let ddim = Ddim::new(25);
        let mut linear = 1.0f64;
        for i in 0..1000 {
            let t = i as f64 / 999.0;
            linear *= 1.0 - (BETA_START + (BETA_END - BETA_START) * t);
        }
        assert!(
            (linear - ddim.alphas_cumprod[999]).abs() > 1e-4,
            "the two schedules should not agree"
        );
    }

    #[test]
    fn the_timesteps_walk_backwards_from_the_noisiest() {
        let ddim = Ddim::new(25);
        assert_eq!(ddim.timesteps().len(), 25);
        // 1000 / 25 = 40, offset by 1: 961, 921, ... 1.
        assert_eq!(ddim.timesteps()[0], 961);
        assert_eq!(*ddim.timesteps().last().unwrap(), 1);
        assert!(ddim.timesteps().windows(2).all(|pair| pair[1] < pair[0]));
    }

    #[test]
    fn fewer_steps_spread_further_apart() {
        assert_eq!(Ddim::new(4).timesteps(), &[751, 501, 251, 1]);
        assert_eq!(Ddim::new(50).timesteps()[0], 981);
    }

    #[test]
    fn adding_no_noise_at_the_cleanest_step_barely_changes_the_sample() {
        let ddim = Ddim::new(25);
        let clean = vec![0.5f32; 8];
        let noise = vec![1.0f32; 8];
        let noised = ddim.add_noise(&clean, &noise, 0);
        // ᾱ ≈ 0.999 at step 0, so the sample is almost untouched.
        assert!((noised[0] - 0.5).abs() < 0.05, "got {}", noised[0]);
    }

    #[test]
    fn adding_noise_at_the_noisiest_step_almost_erases_the_sample() {
        let ddim = Ddim::new(25);
        let noised = ddim.add_noise(&[0.5f32; 8], &[1.0f32; 8], 999);
        // ᾱ ≈ 0.0005, so what is left is nearly all noise.
        assert!(noised[0] > 0.9, "got {}", noised[0]);
    }

    #[test]
    fn a_step_with_perfectly_predicted_noise_recovers_the_clean_sample() {
        // The property that catches a wrong coefficient: noise a known sample,
        // hand the step exactly the noise that was added, and the guess at the
        // clean image must come back. Then re-noising to the previous, gentler
        // level must sit between the two.
        let ddim = Ddim::new(25);
        let clean = vec![0.42f32; 4];
        let noise = vec![-0.8f32; 4];
        let t = 481;

        let noisy = ddim.add_noise(&clean, &noise, t);
        let stepped = ddim.step(&noisy, &noise, t);

        // Recomputing the same way the step does, at the previous level.
        let alpha_prev = ddim.alpha_prev(t);
        let expected =
            (alpha_prev.sqrt() as f32) * 0.42 + ((1.0 - alpha_prev).sqrt() as f32) * -0.8;
        assert!((stepped[0] - expected).abs() < 1e-3, "got {}", stepped[0]);
    }

    #[test]
    fn stepping_moves_towards_the_clean_image() {
        let ddim = Ddim::new(25);
        let clean = vec![0.42f32; 4];
        let noise = vec![-0.8f32; 4];

        let noisy = ddim.add_noise(&clean, &noise, 961);
        let stepped = ddim.step(&noisy, &noise, 961);
        // Less noise left after a step than before it.
        assert!(
            (stepped[0] - 0.42).abs() < (noisy[0] - 0.42).abs(),
            "{} should be closer to 0.42 than {}",
            stepped[0],
            noisy[0]
        );
    }

    #[test]
    fn the_same_seed_gives_the_same_noise() {
        let a = Noise::seeded(7).vector(64);
        let b = Noise::seeded(7).vector(64);
        let c = Noise::seeded(8).vector(64);

        assert_eq!(a, b, "a seed must be reproducible");
        assert_ne!(a, c, "different seeds must differ");
    }

    #[test]
    fn the_noise_is_gaussian_not_uniform() {
        // Uniform noise would run fine and quietly wash every image out.
        let values = Noise::seeded(3).vector(20_000);
        let mean = values.iter().map(|v| *v as f64).sum::<f64>() / values.len() as f64;
        let variance = values
            .iter()
            .map(|v| (*v as f64 - mean).powi(2))
            .sum::<f64>()
            / values.len() as f64;

        assert!(mean.abs() < 0.05, "mean {mean}");
        assert!((variance - 1.0).abs() < 0.08, "variance {variance}");
        // A normal distribution reaches past 3; a uniform one never would.
        assert!(values.iter().any(|v| v.abs() > 3.0));
    }
}
