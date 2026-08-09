/// 参考长边（源像素），该尺寸下自适应默认值 = 锚定的醒目默认值。
const REFERENCE_LONG_SIDE: f32 = 3000.0;

// 醒目默认值（3000px 长边）。
const FONT_SIZE_DEFAULT: f32 = 36.0;
const STROKE_WIDTH_DEFAULT: f32 = 6.0;
const BADGE_SIZE_DEFAULT: f32 = 30.0;

// 与宿主硬校验范围（ui_validation.rs:141-220）及 slider min/max 一致。
const FONT_SIZE_MIN: f32 = 8.0;
const FONT_SIZE_MAX: f32 = 256.0;
const STROKE_WIDTH_MIN: f32 = 1.0;
const STROKE_WIDTH_MAX: f32 = 64.0;
const BADGE_SIZE_MIN: f32 = 12.0;
const BADGE_SIZE_MAX: f32 = 96.0;

const FONT_FRACTION: f32 = FONT_SIZE_DEFAULT / REFERENCE_LONG_SIDE; // 0.012
const STROKE_FRACTION: f32 = STROKE_WIDTH_DEFAULT / REFERENCE_LONG_SIDE; // 0.002
const BADGE_FRACTION: f32 = BADGE_SIZE_DEFAULT / REFERENCE_LONG_SIDE; // 0.01

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AdaptiveDefaults {
    pub font_size: f32,
    pub stroke_width: f32,
    pub badge_size: f32,
}

/// 按图片显示尺寸（已含旋转交换）的长边计算自适应默认值。
///
/// 标注尺寸存储在源像素空间，宿主渲染时乘 scale = display_width / source_width
/// 映射到屏幕。取长边后，屏幕上的标注尺寸 ≈ 固定比例 × 图片长边在屏幕上的投影，
/// 宽图与高图都能获得比例合适的标注，且旋转交换宽高后长边不变、默认值不漂移。
///
/// 会话是一次性进程，`ui.activate` 只触发一次，激活时调用一次即可。
pub fn defaults_for_image(width: u32, height: u32) -> AdaptiveDefaults {
    let long_side = width.max(height).max(1) as f32;
    AdaptiveDefaults {
        font_size: scale_to(long_side, FONT_FRACTION, FONT_SIZE_MIN, FONT_SIZE_MAX),
        stroke_width: scale_to(
            long_side,
            STROKE_FRACTION,
            STROKE_WIDTH_MIN,
            STROKE_WIDTH_MAX,
        ),
        badge_size: scale_to(long_side, BADGE_FRACTION, BADGE_SIZE_MIN, BADGE_SIZE_MAX),
    }
}

fn scale_to(long_side: f32, fraction: f32, min: f32, max: f32) -> f32 {
    (long_side * fraction).round().clamp(min, max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_anchor_to_visible_values_at_3000() {
        assert_eq!(
            defaults_for_image(3000, 3000),
            AdaptiveDefaults {
                font_size: 36.0,
                stroke_width: 6.0,
                badge_size: 30.0,
            }
        );
    }

    #[test]
    fn defaults_key_off_the_longer_axis() {
        let landscape = defaults_for_image(4000, 3000);
        let portrait = defaults_for_image(3000, 4000);
        assert_eq!(landscape, portrait);
        assert_eq!(
            landscape,
            AdaptiveDefaults {
                font_size: 48.0,
                stroke_width: 8.0,
                badge_size: 40.0,
            }
        );

        let tall = defaults_for_image(1000, 8000);
        let wide = defaults_for_image(8000, 1000);
        assert_eq!(tall, wide);
        assert_eq!(
            tall,
            AdaptiveDefaults {
                font_size: 96.0,
                stroke_width: 16.0,
                badge_size: 80.0,
            }
        );
    }

    #[test]
    fn defaults_scale_up_for_large_images() {
        assert_eq!(
            defaults_for_image(8000, 8000),
            AdaptiveDefaults {
                font_size: 96.0,
                stroke_width: 16.0,
                badge_size: 80.0,
            }
        );
    }

    #[test]
    fn defaults_clamp_to_validation_ranges_at_extremes() {
        assert_eq!(
            defaults_for_image(100, 100),
            AdaptiveDefaults {
                font_size: 8.0,
                stroke_width: 1.0,
                badge_size: 12.0,
            }
        );
        assert_eq!(
            defaults_for_image(50000, 50000),
            AdaptiveDefaults {
                font_size: 256.0,
                stroke_width: 64.0,
                badge_size: 96.0,
            }
        );
    }

    #[test]
    fn defaults_handle_zero_dimensions_safely() {
        let defaults = defaults_for_image(0, 0);
        assert!(defaults.font_size.is_finite());
        assert!(defaults.stroke_width.is_finite());
        assert!(defaults.badge_size.is_finite());
        assert_eq!(
            defaults,
            AdaptiveDefaults {
                font_size: 8.0,
                stroke_width: 1.0,
                badge_size: 12.0,
            }
        );
    }

    #[test]
    fn all_defaults_are_finite_and_within_host_ranges() {
        for width in [
            1,
            100,
            1000,
            1500,
            3000,
            8000,
            12000,
            32000,
            50000,
            u32::MAX,
        ] {
            let defaults = defaults_for_image(width, width);
            assert!(defaults.font_size.is_finite(), "font @ {width}");
            assert!(defaults.stroke_width.is_finite(), "stroke @ {width}");
            assert!(defaults.badge_size.is_finite(), "badge @ {width}");
            assert!(
                (FONT_SIZE_MIN..=FONT_SIZE_MAX).contains(&defaults.font_size),
                "font {width} → {}",
                defaults.font_size
            );
            assert!(
                (STROKE_WIDTH_MIN..=STROKE_WIDTH_MAX).contains(&defaults.stroke_width),
                "stroke {width} → {}",
                defaults.stroke_width
            );
            assert!(
                (BADGE_SIZE_MIN..=BADGE_SIZE_MAX).contains(&defaults.badge_size),
                "badge {width} → {}",
                defaults.badge_size
            );
        }
    }
}
