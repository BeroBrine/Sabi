use std::f32::consts::PI;

use crate::fft::complex::Complex;

#[allow(dead_code, non_snake_case)]
pub struct CooleyTukeyFFT {
    CHUNK_SIZE: usize,
    OVERLAP_SIZE: usize,
}

#[allow(dead_code, non_snake_case)]
impl CooleyTukeyFFT {
    pub fn new(CHUNK_SIZE: usize, OVERLAP_SIZE: usize) -> Self {
        if CHUNK_SIZE.is_power_of_two() == false {
            panic!("Chunk Size must be power of two for this implementation to work")
        }

        Self {
            CHUNK_SIZE,
            OVERLAP_SIZE,
        }
    }

    fn apply_hann_window(&self, chunk: &[f32]) -> Vec<f32> {
        let n = chunk.len();
        chunk
            .iter()
            .enumerate()
            .map(|(i, &sample)| {
                let num = 2.0 * PI * (i as f32);
                let denom = (n as f32) - 1.0;
                //window function formula =  w[n] = 0.5 *  cos( 1 - ( (2 * PI * i) / (n - 1) ) )
                let multiplier = 0.5 * (1.0 - (num / denom)).cos();
                sample * multiplier
            })
            .collect()
    }

    fn cooley_tukey_fft(&self, buf: &mut [Complex]) {
        let n = buf.len();

        if n <= 1 {
            return;
        }
        let mut even: Vec<Complex> = Vec::with_capacity(n / 2);
        let mut odd: Vec<Complex> = Vec::with_capacity(n / 2);

        for (i, &sample) in buf.iter().enumerate() {
            if i % 2 == 0 {
                even.push(sample);
            } else {
                odd.push(sample);
            }
        }

        self.cooley_tukey_fft(&mut even);
        self.cooley_tukey_fft(&mut odd);

        // These formula comes from the CooleyTukeyFFT algorithm.
        // Basically to evaluate the audio signal for many sine and cosine waves (fourier transform)
        // Cooley Tukey helps by halving the computation by breaking the parts into even and odd
        // evaluation

        // P(ω)  = Pₑ(ω²) + ωPₒ(ω²)
        // P(-ω) = Pₑ(ω²) - ωPₒ(ω²)
        // where ω = e^i(2π/n) = cos(theta) + i·sin(theta) where theta = 2πk/n // euler's formula
        // -ω^j   = ω^(j + n/2)

        for j in 0..n / 2 {
            let theta = (2.0 * PI * (j as f32)) / (n as f32);
            // negative theta is the convention to write for forward fft. (evaluation)

            // from_polar handles the conversion of euler's formula to complex numbers
            let omega = Complex::from_polar(1.00, -theta);

            // positive evaluation
            buf[j] = even[j] + (omega * odd[j]);
            // negative evaluation as -ω^j   = ω^(j + n/2)
            buf[j + n / 2] = even[j] - (omega * odd[j]);
        }
    }

    fn perform_fft(&self, buff: Vec<f32>) -> Vec<Complex> {
        let mut complex_buff = self.convert_to_complex_buffer(buff);

        self.cooley_tukey_fft(&mut complex_buff);

        complex_buff
    }

    pub fn fingerprint_audio(
        &self,
        buffer: Vec<f32>,
        sample_rate: u32,
    ) -> Vec<(u32, Vec<(f32, f32)>)> {
        let buf_len = buffer.len();
        let mut position = 0;

        let mut fingerprints = Vec::new();

        while position + self.CHUNK_SIZE <= buf_len {
            let chunk = &buffer[position..position + self.CHUNK_SIZE];

            let windowed_chunk = self.apply_hann_window(chunk);

            let fft_output = self.perform_fft(windowed_chunk);

            let peaks = self.find_peaks(&fft_output, sample_rate);

            let time = position as u32 / sample_rate;

            fingerprints.push((time, peaks));

            position += self.CHUNK_SIZE - self.OVERLAP_SIZE;
        }

        fingerprints
    }

    fn find_peaks(&self, complex_buffer: &[Complex], sample_rate: u32) -> Vec<(f32, f32)> {
        let n = complex_buffer.len();
        let half_n = n / 2;

        let magnitudes: Vec<f32> = complex_buffer[..half_n]
            .iter()
            .map(|&c| c.norm_sqr().sqrt())
            .collect();

        let mut peaks: Vec<(f32, f32)> = Vec::new();

        for i in 1..half_n - 1 {
            if magnitudes[i - 1] < magnitudes[i] && magnitudes[i] > magnitudes[i + 1] {
                let freq = i as f32 * (sample_rate as f32 / n as f32);

                // music frequency
                if 300.0 < freq && freq < 20.0 * 1000.0 {
                    peaks.push((freq, magnitudes[i]));
                }
            }
        }

        peaks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        peaks.truncate(5);

        peaks
    }

    fn convert_to_complex_buffer(&self, buffer: Vec<f32>) -> Vec<Complex> {
        buffer
            .iter()
            .map(|&sample| Complex::new(sample, 0.0))
            .collect()
    }
}

impl Default for CooleyTukeyFFT {
    fn default() -> Self {
        Self {
            CHUNK_SIZE: 4096,
            OVERLAP_SIZE: 1024,
        }
    }
}
