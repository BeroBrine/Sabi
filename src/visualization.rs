use std::fs::File;
use std::io::Write;
use std::path::Path;

use crate::fft::fft::{FFTDistribution, FreqRange};

pub fn write_heatmap_svg<P: AsRef<Path>>(
    fingerprints: &Vec<FFTDistribution>,
    output_path: P,
    song_name: &str,
) -> std::io::Result<()> {
    let (width, height) = (1920.0f32, 1080.0f32);

    if fingerprints.is_empty() {
        let empty_svg = format!(
            "<svg xmlns='http://www.w3.org/2000/svg' width='{w}' height='{h}' viewBox='0 0 {w} {h}'>\n  <rect width='100%' height='100%' fill='black'/>\n  <text x='{cx}' y='{cy}' fill='white' font-family='monospace' font-size='20' text-anchor='middle'>No fingerprints</text>\n</svg>",
            w = width,
            h = height,
            cx = width / 2.0,
            cy = height / 2.0
        );
        let mut f = File::create(output_path)?;
        f.write_all(empty_svg.as_bytes())?;
        return Ok(());
    }

    let min_freq = FreqRange::Low.get_freq();
    let max_freq = FreqRange::High.get_freq();

    let max_time = fingerprints
        .last()
        .map(|t| t.time.into_inner())
        .unwrap()
        .max(
            fingerprints
                .iter()
                .map(|t| t.time.into_inner())
                .fold(0.0f32, f32::max),
        );

    // Heat map parameters
    let time_bins = 400; // Number of time bins
    let freq_bins = 200; // Number of frequency bins

    // Create heat map grid
    let mut heatmap = vec![vec![0.0f32; time_bins]; freq_bins];

    // Fill heat map with data
    for fingerprint in fingerprints.iter() {
        let time = fingerprint.time.into_inner();
        let time_bin = ((time / max_time) * (time_bins - 1) as f32)
            .clamp(0.0, (time_bins - 1) as f32) as usize;

        for peak in &fingerprint.peaks {
            let freq = peak.freq.into_inner();
            let mag = peak.magnitude.into_inner();

            if freq >= min_freq && freq <= max_freq && mag.is_finite() {
                let freq_bin = (((freq - min_freq) / (max_freq - min_freq))
                    * (freq_bins - 1) as f32)
                    .clamp(0.0, (freq_bins - 1) as f32) as usize;

                // Accumulate magnitude in the bin (use max to avoid double counting)
                heatmap[freq_bin][time_bin] = heatmap[freq_bin][time_bin].max(mag);
            }
        }
    }

    // Find maximum magnitude for normalization
    let max_mag = heatmap
        .iter()
        .flatten()
        .fold(0.0f32, |acc, &val| acc.max(val));

    if max_mag <= 0.0 {
        let empty_svg = format!(
            "<svg xmlns='http://www.w3.org/2000/svg' width='{w}' height='{h}' viewBox='0 0 {w} {h}'>\n  <rect width='100%' height='100%' fill='black'/>\n  <text x='{cx}' y='{cy}' fill='white' font-family='monospace' font-size='20' text-anchor='middle'>No data to visualize</text>\n</svg>",
            w = width,
            h = height,
            cx = width / 2.0,
            cy = height / 2.0
        );
        let mut f = File::create(output_path)?;
        f.write_all(empty_svg.as_bytes())?;
        return Ok(());
    }

    let padding_left = 170.0f32; // extra space for y-axis labels
    let padding_right = 50.0f32;
    let padding_top = 70.0f32; // space for title
    let padding_bottom = 60.0f32; // space for x-axis label
    let plot_w = width - padding_left - padding_right;
    let plot_h = height - padding_top - padding_bottom;

    let bg = format!(
        "<rect x='0' y='0' width='{w}' height='{h}' fill='black'/>",
        w = width,
        h = height
    );

    let axes = format!(
        "<g stroke='white' stroke-width='1' opacity='0.6'>\n  <line x1='{px}' y1='{py}' x2='{px}' y2='{py2}'/>\n  <line x1='{px}' y1='{py2}' x2='{px2}' y2='{py2}'/>\n</g>",
        px = padding_left,
        py = padding_top,
        px2 = padding_left + plot_w,
        py2 = padding_top + plot_h,
    );

    // y-axis ticks and labels
    let tick_freqs: [f32; 5] = [300.0, 500.0, 1000.0, 2000.0, 5000.0];
    let mut y_ticks = String::new();
    for f in tick_freqs.iter() {
        let y = {
            let clamped = f.clamp(min_freq, max_freq);
            let norm = (clamped - min_freq) / (max_freq - min_freq);
            padding_top + (1.0 - norm) * plot_h
        };
        y_ticks.push_str(&format!(
            "<g>\n  <line x1='{x1:.2}' y1='{y:.2}' x2='{x2:.2}' y2='{y:.2}' stroke='white' stroke-opacity='0.25' stroke-width='1'/>\n  <text x='{tx:.2}' y='{ty:.2}' fill='white' font-family='monospace' font-size='11' text-anchor='end'>{label}</text>\n</g>\n",
            x1 = padding_left - 6.0,
            x2 = padding_left + plot_w,
            y = y,
            tx = padding_left - 10.0,
            ty = y + 4.0,
            label = format!("{} Hz", *f as i32)
        ));
    }

    // Generate heat map rectangles
    let cell_width = plot_w / time_bins as f32;
    let cell_height = plot_h / freq_bins as f32;

    let mut heatmap_rects = String::new();

    for (freq_idx, freq_row) in heatmap.iter().enumerate() {
        for (time_idx, &magnitude) in freq_row.iter().enumerate() {
            if magnitude > 0.0 {
                let x = padding_left + time_idx as f32 * cell_width;
                let y = padding_top + (freq_bins - 1 - freq_idx) as f32 * cell_height;

                // Normalize magnitude and convert to color
                let normalized_mag = (magnitude / max_mag).clamp(0.0, 1.0);
                let color = magnitude_to_color(normalized_mag);

                heatmap_rects.push_str(&format!(
                    "<rect x='{x:.2}' y='{y:.2}' width='{w:.2}' height='{h:.2}' fill='{color}'/>\n",
                    x = x,
                    y = y,
                    w = cell_width,
                    h = cell_height,
                    color = color
                ));
            }
        }
    }

    let labels = format!(
        "<g fill='white' font-family='monospace' font-size='12'>\n  <text x='{px}' y='{py}' text-anchor='start'>freq: {min} Hz → {max} Hz</text>\n  <text x='{px}' y='{py2}' dy='20' text-anchor='start'>time: 0 → {tmax:.2}s</text>\n</g>",
        px = padding_left,
        py = padding_top - 10.0,
        py2 = padding_top + plot_h,
        min = min_freq as i32,
        max = max_freq as i32,
        tmax = max_time,
    );

    let title = format!(
        "<text x='{x}' y='{y}' fill='white' font-family='monospace' font-size='14' text-anchor='end'>{name}</text>",
        x = padding_left + plot_w,
        y = padding_top - 20.0,
        name = svg_escape(song_name)
    );

    let svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='{w}' height='{h}' viewBox='0 0 {w} {h}'>\n{bg}\n{axes}\n{y_ticks}<g>\n{heatmap_rects}</g>\n{labels}\n{title}\n</svg>",
        w = width,
        h = height,
        bg = bg,
        axes = axes,
        y_ticks = y_ticks,
        heatmap_rects = heatmap_rects,
        labels = labels,
        title = title,
    );

    let mut file = File::create(output_path)?;
    file.write_all(svg.as_bytes())?;
    Ok(())
}

fn magnitude_to_color(normalized_mag: f32) -> String {
    // Create a color gradient from black (low) to bright colors (high)
    // Using a perceptually uniform color scheme: black -> blue -> cyan -> yellow -> red

    if normalized_mag <= 0.0 {
        return "#000000".to_string(); // Black for no data
    }

    let clamped = normalized_mag.clamp(0.0, 1.0);

    if clamped < 0.2 {
        // Black to dark blue
        let intensity = (clamped / 0.2) * 0.3;
        format!("#{:02x}{:02x}{:02x}", 0, 0, (intensity * 255.0) as u8)
    } else if clamped < 0.4 {
        // Dark blue to blue
        let intensity = ((clamped - 0.2) / 0.2) * 0.5 + 0.3;
        format!("#{:02x}{:02x}{:02x}", 0, 0, (intensity * 255.0) as u8)
    } else if clamped < 0.6 {
        // Blue to cyan
        let intensity = ((clamped - 0.4) / 0.2) * 0.5 + 0.5;
        let green = (intensity * 255.0) as u8;
        format!("#{:02x}{:02x}{:02x}", 0, green, 255)
    } else if clamped < 0.8 {
        // Cyan to yellow
        let intensity = ((clamped - 0.6) / 0.2) * 0.5 + 0.5;
        let red = (intensity * 255.0) as u8;
        let green = 255;
        let blue = ((1.0 - intensity) * 255.0) as u8;
        format!("#{:02x}{:02x}{:02x}", red, green, blue)
    } else {
        // Yellow to red
        let intensity = ((clamped - 0.8) / 0.2) * 0.5 + 0.5;
        let red = 255;
        let green = ((1.0 - intensity) * 255.0) as u8;
        let blue = 0;
        format!("#{:02x}{:02x}{:02x}", red, green, blue)
    }
}

fn svg_escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '&' => out.push_str("&amp;"),
            _ => out.push(ch),
        }
    }
    out
}
