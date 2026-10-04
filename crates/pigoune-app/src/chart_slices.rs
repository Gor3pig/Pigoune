use gettextrs::gettext;
use pigoune_core::{AssetFormat, FormatShare};

use crate::asset_facts;

const SMALL_SHARE: f64 = 0.03;
const OTHERS_COLOR: &str = "#9a9996";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Measure {
    Weight,
    Count,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Slice {
    pub label: String,
    pub color: &'static str,
    pub count: usize,
    pub bytes: u64,
    pub fraction: f64,
}

pub fn slices(shares: &[FormatShare], measure: Measure) -> Vec<Slice> {
    let value = |share: &FormatShare| match measure {
        Measure::Weight => share.bytes,
        Measure::Count => u64::try_from(share.count).unwrap_or(u64::MAX),
    };
    let total: u64 = shares.iter().map(value).sum();
    if total == 0 {
        return Vec::new();
    }
    let fraction_of = |amount: u64| ratio(amount, total);
    let mut sorted: Vec<&FormatShare> = shares.iter().filter(|share| value(share) > 0).collect();
    sorted.sort_by_key(|share| std::cmp::Reverse(value(share)));
    let (large, small): (Vec<&FormatShare>, Vec<&FormatShare>) = sorted
        .into_iter()
        .partition(|share| fraction_of(value(share)) >= SMALL_SHARE);
    let mut slices: Vec<Slice> = large
        .iter()
        .map(|share| Slice {
            label: asset_facts::format_name(share.format).to_owned(),
            color: color_of(share.format),
            count: share.count,
            bytes: share.bytes,
            fraction: fraction_of(value(share)),
        })
        .collect();
    match small.as_slice() {
        [] => {}
        [single] => slices.push(Slice {
            label: asset_facts::format_name(single.format).to_owned(),
            color: color_of(single.format),
            count: single.count,
            bytes: single.bytes,
            fraction: fraction_of(value(single)),
        }),
        several => slices.push(Slice {
            label: gettext("Others"),
            color: OTHERS_COLOR,
            count: several.iter().map(|share| share.count).sum(),
            bytes: several.iter().map(|share| share.bytes).sum(),
            fraction: fraction_of(several.iter().map(|share| value(share)).sum()),
        }),
    }
    slices
}

#[expect(
    clippy::cast_precision_loss,
    reason = "a share only needs to be precise enough to be drawn"
)]
pub fn ratio(part: u64, whole: u64) -> f64 {
    if whole == 0 {
        0.0
    } else {
        part as f64 / whole as f64
    }
}

pub fn color_of(format: AssetFormat) -> &'static str {
    match format {
        AssetFormat::Svg => "#3584e4",
        AssetFormat::Png => "#2190a4",
        AssetFormat::Jpeg => "#3a944a",
        AssetFormat::Webp => "#c88800",
        AssetFormat::Avif => "#ed5b00",
        AssetFormat::Jxl => "#e62d42",
        AssetFormat::Gif => "#d56199",
        AssetFormat::Tiff => "#9141ac",
        AssetFormat::Bmp => "#865e3c",
        AssetFormat::Ico => "#6f8396",
    }
}

#[cfg(test)]
mod tests {
    use pigoune_core::{AssetFormat, FormatShare};

    use super::{Measure, slices};

    fn share(format: AssetFormat, count: usize, bytes: u64) -> FormatShare {
        FormatShare {
            format,
            count,
            bytes,
        }
    }

    fn labels(shares: &[FormatShare], measure: Measure) -> Vec<(String, String)> {
        slices(shares, measure)
            .into_iter()
            .map(|slice| (slice.label, format!("{:.1}", slice.fraction * 100.0)))
            .collect()
    }

    fn pair(label: &str, percent: &str) -> (String, String) {
        (label.to_owned(), percent.to_owned())
    }

    #[test]
    fn slices_are_sorted_from_largest_to_smallest() {
        let shares = [
            share(AssetFormat::Svg, 90, 100),
            share(AssetFormat::Png, 10, 900),
        ];
        assert_eq!(
            labels(&shares, Measure::Weight),
            [pair("PNG", "90.0"), pair("SVG", "10.0")]
        );
        assert_eq!(
            labels(&shares, Measure::Count),
            [pair("SVG", "90.0"), pair("PNG", "10.0")]
        );
    }

    #[test]
    fn several_small_shares_are_gathered_as_others() {
        let shares = [
            share(AssetFormat::Png, 1, 960),
            share(AssetFormat::Gif, 1, 20),
            share(AssetFormat::Bmp, 1, 20),
        ];
        let result = slices(&shares, Measure::Weight);
        assert_eq!(result.len(), 2);
        assert_eq!(result[1].label, "Others");
        assert_eq!((result[1].count, result[1].bytes), (2, 40));
    }

    #[test]
    fn a_single_small_share_keeps_its_own_name() {
        let shares = [
            share(AssetFormat::Png, 1, 980),
            share(AssetFormat::Gif, 1, 20),
        ];
        let result = slices(&shares, Measure::Weight);
        assert_eq!(result[1].label, "GIF");
    }

    #[test]
    fn an_empty_library_has_no_slice() {
        assert!(slices(&[], Measure::Weight).is_empty());
    }
}
