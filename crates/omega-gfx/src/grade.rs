const WARMEST: [f32; 3] = [1.0, 0.82, 0.62];

pub const BLUE_LIGHT_MAX: u8 = 9;

/// How bright the picture itself is drawn at brightness steps 0, 1 and 2, where the backlight
/// is as low as it goes or near it: the picture dims the rest of the way. From step 3 up the
/// backlight alone sets it, as it always did.
const LOW_LIGHT: [f32; 3] = [0.4, 0.6, 0.8];

/// How bright the picture is drawn at brightness `step`, of `BLUE_LIGHT_MAX`'s nine.
pub fn low_light(step: u8) -> f32 {
    LOW_LIGHT.get(usize::from(step)).copied().unwrap_or(1.0)
}

/// The picture's gain for blue light filter `blue_light` at brightness `brightness`.
pub fn grade_gain(blue_light: u8, brightness: u8) -> [f32; 3] {
    blue_light_gain(blue_light).map(|g| g * low_light(brightness))
}

pub fn blue_light_gain(step: u8) -> [f32; 3] {
    let t = step.min(BLUE_LIGHT_MAX) as f32 / BLUE_LIGHT_MAX as f32;
    let mut gain = [1.0f32; 3];
    for (g, warm) in gain.iter_mut().zip(WARMEST) {
        *g = 1.0 + (warm - 1.0) * t;
    }
    gain
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_three_lowest_steps_dim_the_picture() {
        assert!(low_light(0) < low_light(1) && low_light(1) < low_light(2));
        assert!(low_light(2) < 1.0);
        for step in 3..=9 {
            assert_eq!(low_light(step), 1.0, "step {step}");
        }
        assert_eq!(grade_gain(0, 9), [1.0; 3]);
        assert_eq!(grade_gain(4, 5), blue_light_gain(4));
    }
}
