use std::io::{Read, Seek};
use std::ops::Range;
use std::path::Path;

use crate::raster::RgbaImage;

use super::xcf::{
    BaseType, LayerInfo, LayerKind, LayerRole, SampleGrid, Samples, XcfError, XcfReader,
};
use super::xcf_blend::{Blend, Composite, Rgb, Space, from_linear, to_linear, to_linear_from};

const BAND_ROWS: usize = 128;
const MOST_BAND_PIXELS: usize = 1 << 20;
const MOST_GROUP_DEPTH: usize = 16;
const CHANNELS: usize = 4;
const LEGACY_MODES_END: u32 = 23;
const BYTE_MAX: f32 = 255.0;
const WORD_MAX: f32 = 65_535.0;
const MOST_PIXEL_VISITS: usize = 600_000_000;

#[derive(Debug, Clone, PartialEq)]
pub struct GimpRender {
    pub image: RgbaImage,
    pub approximate: bool,
}

#[derive(Debug, Clone, Copy)]
struct Rules {
    blend: Blend,
    blend_space: Space,
    composite_space: Space,
    composite: Composite,
    clamp: bool,
}

struct Node {
    index: usize,
    children: Vec<Node>,
}

struct Band {
    rows: Range<usize>,
}

#[derive(Clone)]
struct Buffer {
    width: usize,
    data: Vec<f32>,
}

impl Buffer {
    fn transparent(width: usize, rows: usize) -> Self {
        Self {
            width,
            data: vec![0.0; width * rows * CHANNELS],
        }
    }

    fn pixel(&self, offset: usize) -> [f32; CHANNELS] {
        [
            self.data[offset],
            self.data[offset + 1],
            self.data[offset + 2],
            self.data[offset + 3],
        ]
    }

    fn set(&mut self, offset: usize, pixel: [f32; CHANNELS]) {
        self.data[offset..offset + CHANNELS].copy_from_slice(&pixel);
    }

    fn scale_alpha(&mut self, factor: f32) {
        for pixel in self.data.as_chunks_mut::<CHANNELS>().0 {
            pixel[3] *= factor;
        }
    }
}

struct Transfer {
    linear: bool,
    table: [f32; 256],
}

impl Transfer {
    fn new(linear: bool) -> Self {
        let mut table = [0.0_f32; 256];
        for (value, slot) in (0_u16..).zip(table.iter_mut()) {
            let normalised = f32::from(value) / BYTE_MAX;
            *slot = if linear {
                normalised
            } else {
                to_linear(normalised)
            };
        }
        Self { linear, table }
    }

    fn float(&self, value: f32) -> f32 {
        if self.linear { value } else { to_linear(value) }
    }
}

struct Painter<'a, R> {
    reader: &'a mut XcfReader<R>,
    layers: Vec<LayerInfo>,
    width: usize,
    base: BaseType,
    palette: Vec<[u8; 3]>,
    transfer: Transfer,
    approximate: bool,
    visits: usize,
}

pub fn render_gimp_file(path: &Path) -> Result<GimpRender, XcfError> {
    compose_image(&mut XcfReader::open(path)?)
}

#[must_use]
pub fn gimp_file_is_intact(path: &Path) -> bool {
    !matches!(render_gimp_file(path), Err(XcfError::Unreadable))
}

#[must_use]
pub fn gimp_problem(path: &Path) -> Option<XcfError> {
    render_gimp_file(path).err()
}

pub fn compose_image<R: Read + Seek>(reader: &mut XcfReader<R>) -> Result<GimpRender, XcfError> {
    let document = reader.document();
    let (width, height) = (document.width, document.height);
    let layers = document.layers.clone();
    let tree = build_tree(&layers)?;
    let mut painter = Painter {
        width,
        base: document.base_type,
        palette: document.palette.clone(),
        transfer: Transfer::new(document.precision.linear),
        approximate: false,
        visits: 0,
        layers,
        reader,
    };
    let mut pixels = vec![0_u8; width * height * CHANNELS];
    let band_rows = (MOST_BAND_PIXELS / width.max(1)).clamp(1, BAND_ROWS);
    let mut first = 0;
    while first < height {
        let band = Band {
            rows: first..(first + band_rows).min(height),
        };
        let mut canvas = Buffer::transparent(width, band.rows.len());
        painter.render_nodes(&tree, &band, &mut canvas)?;
        let start = first * width * CHANNELS;
        encode_band(
            &canvas,
            &mut pixels[start..start + canvas.data.len() / CHANNELS * CHANNELS],
        );
        first = band.rows.end;
    }
    let image = u32::try_from(width)
        .ok()
        .zip(u32::try_from(height).ok())
        .and_then(|(width, height)| RgbaImage::new(width, height, pixels))
        .ok_or(XcfError::Unreadable)?;
    Ok(GimpRender {
        image,
        approximate: painter.approximate,
    })
}

fn build_tree(layers: &[LayerInfo]) -> Result<Vec<Node>, XcfError> {
    let mut top_level_rank = 0_u32;
    let paths: Vec<Vec<u32>> = layers
        .iter()
        .map(|layer| {
            if layer.path.is_empty() {
                top_level_rank += 1;
                vec![top_level_rank - 1]
            } else {
                layer.path.clone()
            }
        })
        .collect();
    children_of(layers, &paths, &[])
}

fn children_of(
    layers: &[LayerInfo],
    paths: &[Vec<u32>],
    parent: &[u32],
) -> Result<Vec<Node>, XcfError> {
    if parent.len() > MOST_GROUP_DEPTH {
        return Err(XcfError::TooLarge);
    }
    let mut members: Vec<(u32, usize)> = paths
        .iter()
        .enumerate()
        .filter(|(_, path)| path.len() == parent.len() + 1 && path.starts_with(parent))
        .map(|(index, path)| (path[path.len() - 1], index))
        .collect();
    members.sort_unstable();
    members.dedup_by_key(|member| member.0);
    members
        .into_iter()
        .map(|(_, index)| {
            let children = if layers[index].role == LayerRole::Group {
                children_of(layers, paths, &paths[index])?
            } else {
                Vec::new()
            };
            Ok(Node { index, children })
        })
        .collect()
}

fn rules_for(layer: &LayerInfo) -> (Rules, bool) {
    let known = Blend::from_code(layer.mode);
    let blend = known.unwrap_or(Blend::Normal);
    let legacy = layer.mode < LEGACY_MODES_END;
    let default_composite_space = if legacy {
        Space::Perceptual
    } else {
        Space::Linear
    };
    let rules = Rules {
        blend,
        blend_space: Space::from_stored(layer.blend_space)
            .unwrap_or_else(|| blend.default_blend_space()),
        composite_space: Space::from_stored(layer.composite_space)
            .unwrap_or(default_composite_space),
        composite: Composite::from_stored(layer.composite_mode)
            .unwrap_or_else(|| blend.default_composite()),
        clamp: legacy,
    };
    (rules, known.is_none() || layer.has_filters)
}

impl<R: Read + Seek> Painter<'_, R> {
    fn render_nodes(
        &mut self,
        nodes: &[Node],
        band: &Band,
        target: &mut Buffer,
    ) -> Result<(), XcfError> {
        for node in nodes.iter().rev() {
            let layer = self.layers[node.index].clone();
            if !layer.visible {
                continue;
            }
            self.visits += target.data.len() / CHANNELS;
            if self.visits > MOST_PIXEL_VISITS {
                return Err(XcfError::TooLarge);
            }
            let (rules, approximate) = rules_for(&layer);
            self.approximate |= approximate;
            match layer.role {
                LayerRole::Layer => self.paint_layer(node.index, &layer, rules, band, target)?,
                LayerRole::Group => {
                    self.paint_group(node, &layer, rules, band, target)?;
                }
            }
        }
        Ok(())
    }

    fn paint_layer(
        &mut self,
        index: usize,
        layer: &LayerInfo,
        rules: Rules,
        band: &Band,
        target: &mut Buffer,
    ) -> Result<(), XcfError> {
        let Some(mut source) = self.layer_buffer(index, layer, band)? else {
            return Ok(());
        };
        source.scale_alpha(layer.opacity);
        self.apply_mask(&mut source, index, layer, band)?;
        composite_buffers(target, &source, rules);
        Ok(())
    }

    fn paint_group(
        &mut self,
        node: &Node,
        layer: &LayerInfo,
        rules: Rules,
        band: &Band,
        target: &mut Buffer,
    ) -> Result<(), XcfError> {
        if rules.blend == Blend::PassThrough {
            let mut after = target.clone();
            self.render_nodes(&node.children, band, &mut after)?;
            let mut weights = Buffer::transparent(target.width, band.rows.len());
            for pixel in weights.data.as_chunks_mut::<CHANNELS>().0 {
                pixel[3] = layer.opacity;
            }
            self.apply_mask(&mut weights, node.index, layer, band)?;
            mix_buffers(target, &after, &weights);
            return Ok(());
        }
        let mut inner = Buffer::transparent(target.width, band.rows.len());
        self.render_nodes(&node.children, band, &mut inner)?;
        inner.scale_alpha(layer.opacity);
        self.apply_mask(&mut inner, node.index, layer, band)?;
        composite_buffers(target, &inner, rules);
        Ok(())
    }

    fn layer_buffer(
        &mut self,
        index: usize,
        layer: &LayerInfo,
        band: &Band,
    ) -> Result<Option<Buffer>, XcfError> {
        let columns = visible_columns(layer, self.width);
        let Some((layer_rows, first_row)) = overlap(layer, band).filter(|_| !columns.is_empty())
        else {
            return Ok(None);
        };
        self.visits += layer.width.saturating_sub(columns.len()) * layer_rows.len();
        if self.visits > MOST_PIXEL_VISITS {
            return Err(XcfError::TooLarge);
        }
        let grid = self.reader.layer_rows(index, layer_rows.clone())?;
        let mut buffer = Buffer::transparent(self.width, band.rows.len());
        let first_x =
            usize::try_from(i64::from(layer.offset_x) + i64::try_from(columns.start).unwrap_or(0))
                .unwrap_or(0);
        for row in 0..grid.height {
            let start = ((first_row + row) * self.width + first_x) * CHANNELS;
            let target = &mut buffer.data[start..start + columns.len() * CHANNELS];
            self.decode_row(layer.kind, &grid, row, columns.clone(), target);
        }
        Ok(Some(buffer))
    }

    fn apply_mask(
        &mut self,
        buffer: &mut Buffer,
        index: usize,
        layer: &LayerInfo,
        band: &Band,
    ) -> Result<(), XcfError> {
        if !layer.has_mask() || !layer.apply_mask {
            return Ok(());
        }
        let Some((layer_rows, first_row)) = overlap(layer, band) else {
            buffer.scale_alpha(0.0);
            return Ok(());
        };
        let Some(mask) = self.reader.mask_rows(index, layer_rows)? else {
            return Ok(());
        };
        let columns = visible_columns(layer, self.width);
        let first_x =
            usize::try_from(i64::from(layer.offset_x) + i64::try_from(columns.start).unwrap_or(0))
                .unwrap_or(0);
        for row in 0..band.rows.len() {
            let line =
                &mut buffer.data[row * self.width * CHANNELS..(row + 1) * self.width * CHANNELS];
            let mask_row = row.checked_sub(first_row).filter(|row| *row < mask.height);
            let Some(mask_row) = mask_row else {
                line.fill(0.0);
                continue;
            };
            for (x, pixel) in line.as_chunks_mut::<CHANNELS>().0.iter_mut().enumerate() {
                let column = x.checked_sub(first_x).map(|offset| offset + columns.start);
                let coverage = column
                    .filter(|column| columns.contains(column))
                    .map_or(0.0, |column| {
                        mask_value(&mask, mask_row * mask.width + column)
                    });
                pixel[3] *= coverage;
            }
        }
        Ok(())
    }

    fn decode_row(
        &self,
        kind: LayerKind,
        grid: &SampleGrid,
        row: usize,
        columns: Range<usize>,
        target: &mut [f32],
    ) {
        let row_start = row * grid.width;
        if let (Samples::Eight(data), BaseType::Rgb) = (&grid.samples, self.base) {
            let table = &self.transfer.table;
            for (column, pixel) in columns.zip(target.as_chunks_mut::<CHANNELS>().0.iter_mut()) {
                let at = (row_start + column) * grid.channels;
                pixel[0] = table[usize::from(data[at])];
                pixel[1] = table[usize::from(data[at + 1])];
                pixel[2] = table[usize::from(data[at + 2])];
                pixel[3] = if kind.has_alpha() {
                    f32::from(data[at + 3]) / BYTE_MAX
                } else {
                    1.0
                };
            }
            return;
        }
        for (column, pixel) in columns.zip(target.as_chunks_mut::<CHANNELS>().0.iter_mut()) {
            pixel.copy_from_slice(&self.decode_pixel(kind, grid, row_start + column));
        }
    }

    fn decode_pixel(&self, kind: LayerKind, grid: &SampleGrid, pixel: usize) -> [f32; CHANNELS] {
        let base = pixel * grid.channels;
        let alpha = if kind.has_alpha() {
            alpha_value(&grid.samples, base + grid.channels - 1)
        } else {
            1.0
        };
        let color = match self.base {
            BaseType::Rgb => [
                self.color_value(&grid.samples, base),
                self.color_value(&grid.samples, base + 1),
                self.color_value(&grid.samples, base + 2),
            ],
            BaseType::Gray => [self.color_value(&grid.samples, base); 3],
            BaseType::Indexed => self.palette_color(&grid.samples, base),
        };
        [color[0], color[1], color[2], alpha]
    }

    fn palette_color(&self, samples: &Samples, at: usize) -> Rgb {
        let Samples::Eight(data) = samples else {
            return [0.0; 3];
        };
        let entry = self
            .palette
            .get(usize::from(data[at]))
            .copied()
            .unwrap_or([0; 3]);
        entry.map(|value| self.transfer.table[usize::from(value)])
    }

    fn color_value(&self, samples: &Samples, at: usize) -> f32 {
        match samples {
            Samples::Eight(data) => self.transfer.table[usize::from(data[at])],
            Samples::Sixteen(data) => self.transfer.float(f32::from(data[at]) / WORD_MAX),
            Samples::Float(data) => self.transfer.float(data[at]),
        }
    }
}

fn overlap(layer: &LayerInfo, band: &Band) -> Option<(Range<usize>, usize)> {
    let top = i64::from(layer.offset_y);
    let bottom = top + i64::try_from(layer.height).ok()?;
    let start = i64::try_from(band.rows.start).ok()?.max(top);
    let end = i64::try_from(band.rows.end).ok()?.min(bottom);
    if start >= end {
        return None;
    }
    let first_row = usize::try_from(start - i64::try_from(band.rows.start).ok()?).ok()?;
    Some((
        usize::try_from(start - top).ok()?..usize::try_from(end - top).ok()?,
        first_row,
    ))
}

fn visible_columns(layer: &LayerInfo, canvas_width: usize) -> Range<usize> {
    let left = i64::from(layer.offset_x);
    let first = (-left).max(0);
    let last = (i64::try_from(canvas_width).unwrap_or(0) - left)
        .min(i64::try_from(layer.width).unwrap_or(0));
    if first >= last {
        return 0..0;
    }
    usize::try_from(first).unwrap_or(0)..usize::try_from(last).unwrap_or(0)
}

fn alpha_value(samples: &Samples, at: usize) -> f32 {
    match samples {
        Samples::Eight(data) => f32::from(data[at]) / BYTE_MAX,
        Samples::Sixteen(data) => f32::from(data[at]) / WORD_MAX,
        Samples::Float(data) => data[at],
    }
}

fn mask_value(mask: &SampleGrid, at: usize) -> f32 {
    alpha_value(&mask.samples, at).clamp(0.0, 1.0)
}

fn composite_buffers(target: &mut Buffer, source: &Buffer, rules: Rules) {
    for offset in (0..target.data.len()).step_by(CHANNELS) {
        let layer = source.pixel(offset);
        if layer[3] <= 0.0
            && matches!(
                rules.composite,
                Composite::Union | Composite::ClipToBackdrop
            )
        {
            continue;
        }
        let backdrop = target.pixel(offset);
        target.set(offset, composite_pixel(rules, backdrop, layer));
    }
}

fn composite_pixel(
    rules: Rules,
    backdrop: [f32; CHANNELS],
    layer: [f32; CHANNELS],
) -> [f32; CHANNELS] {
    if rules.blend == Blend::Behind {
        let plain = Rules {
            blend: Blend::Normal,
            composite: Composite::Union,
            ..rules
        };
        return composite_pixel(plain, layer, backdrop);
    }
    let (backdrop_alpha, layer_alpha) = (backdrop[3], layer[3]);
    if rules.blend == Blend::Erase {
        return [
            backdrop[0],
            backdrop[1],
            backdrop[2],
            backdrop_alpha * (1.0 - layer_alpha),
        ];
    }
    let space = rules.composite_space;
    let below = from_linear(space, [backdrop[0], backdrop[1], backdrop[2]]);
    let above = from_linear(space, [layer[0], layer[1], layer[2]]);
    let blended = if rules.blend == Blend::Normal || backdrop_alpha <= 0.0 {
        above
    } else {
        blend_colors(rules, below, above)
    };
    let mixed = mix_colors(above, blended, backdrop_alpha);
    let (alpha, color) = match rules.composite {
        Composite::Union => {
            let alpha = layer_alpha + backdrop_alpha - layer_alpha * backdrop_alpha;
            let kept = (1.0 - layer_alpha) * backdrop_alpha;
            (alpha, combine(below, kept, mixed, layer_alpha, alpha))
        }
        Composite::ClipToBackdrop => (
            backdrop_alpha,
            combine(below, 1.0 - layer_alpha, mixed, layer_alpha, 1.0),
        ),
        Composite::ClipToLayer => (layer_alpha, mixed),
        Composite::Intersection => (layer_alpha * backdrop_alpha, mixed),
    };
    if alpha <= 0.0 {
        return [0.0; CHANNELS];
    }
    let linear = to_linear_from(space, color);
    [linear[0], linear[1], linear[2], alpha]
}

fn blend_colors(rules: Rules, below: Rgb, above: Rgb) -> Rgb {
    let space = rules.composite_space;
    if rules.blend_space == space {
        return clamped(rules, rules.blend.apply(below, above));
    }
    let back = from_linear(rules.blend_space, to_linear_from(space, below));
    let front = from_linear(rules.blend_space, to_linear_from(space, above));
    let result = clamped(rules, rules.blend.apply(back, front));
    from_linear(space, to_linear_from(rules.blend_space, result))
}

fn clamped(rules: Rules, color: Rgb) -> Rgb {
    if rules.clamp {
        color.map(|value| value.clamp(0.0, 1.0))
    } else {
        color
    }
}

fn mix_colors(above: Rgb, blended: Rgb, backdrop_alpha: f32) -> Rgb {
    [
        above[0] + (blended[0] - above[0]) * backdrop_alpha,
        above[1] + (blended[1] - above[1]) * backdrop_alpha,
        above[2] + (blended[2] - above[2]) * backdrop_alpha,
    ]
}

fn combine(below: Rgb, below_weight: f32, mixed: Rgb, layer_alpha: f32, total: f32) -> Rgb {
    let sum = |index: usize| {
        if total <= 0.0 {
            0.0
        } else {
            (below[index] * below_weight + mixed[index] * layer_alpha) / total
        }
    };
    [sum(0), sum(1), sum(2)]
}

fn mix_buffers(target: &mut Buffer, after: &Buffer, weights: &Buffer) {
    for offset in (0..target.data.len()).step_by(CHANNELS) {
        let weight = weights.data[offset + 3].clamp(0.0, 1.0);
        if weight <= 0.0 {
            continue;
        }
        let before = target.pixel(offset);
        let changed = after.pixel(offset);
        let alpha = before[3] * (1.0 - weight) + changed[3] * weight;
        if alpha <= 0.0 {
            target.set(offset, [0.0; CHANNELS]);
            continue;
        }
        let channel = |index: usize| {
            (before[index] * before[3] * (1.0 - weight) + changed[index] * changed[3] * weight)
                / alpha
        };
        target.set(offset, [channel(0), channel(1), channel(2), alpha]);
    }
}

fn encode_band(canvas: &Buffer, output: &mut [u8]) {
    for (pixel, bytes) in canvas
        .data
        .as_chunks::<CHANNELS>()
        .0
        .iter()
        .zip(output.as_chunks_mut::<CHANNELS>().0.iter_mut())
    {
        let alpha = pixel[3].clamp(0.0, 1.0);
        if alpha <= 0.0 {
            bytes.copy_from_slice(&[0; CHANNELS]);
            continue;
        }
        let color = from_linear(Space::Perceptual, [pixel[0], pixel[1], pixel[2]]);
        for (slot, value) in bytes.iter_mut().zip(color) {
            *slot = to_byte(value);
        }
        bytes[3] = to_byte(alpha);
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn to_byte(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * BYTE_MAX + 0.5) as u8
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::compose_image;
    use crate::media::xcf::XcfReader;

    const IMAGE_HEADER_LENGTH: usize = 22;
    const FIRST_FIXTURES: [&str; 8] = [
        "basic", "p16", "p32", "tiles", "gray", "indexed", "mask", "group",
    ];

    fn fixtures() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }

    fn composed(name: &str) -> PathBuf {
        fixtures().join(format!("gimp-composed/{name}.xcf"))
    }

    fn reference(name: &str) -> Vec<u8> {
        let packed =
            std::fs::read(fixtures().join(format!("gimp-composed/{name}.rgba.z"))).unwrap();
        miniz_oxide::inflate::decompress_to_vec_zlib(&packed).unwrap()
    }

    fn tolerance(name: &str) -> i32 {
        if name == "s_prec_u8_linear" { 4 } else { 1 }
    }

    fn assert_same_as_gimp(source: &Path, name: &str) {
        let bytes = std::fs::read(source).unwrap();
        let mut reader = XcfReader::from_bytes(&bytes).unwrap();
        let rendered = compose_image(&mut reader).unwrap();
        let expected = reference(name);
        let actual = rendered.image.pixels();
        assert_eq!(actual.len(), expected.len(), "{name}: size");
        let worst = actual
            .iter()
            .zip(&expected)
            .map(|(a, b)| (i32::from(*a) - i32::from(*b)).abs())
            .max()
            .unwrap();
        assert!(
            worst <= tolerance(name),
            "{name}: differs from GIMP by up to {worst}"
        );
    }

    fn composed_names() -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(fixtures().join("gimp-composed"))
            .unwrap()
            .filter_map(|entry| {
                let file = entry.unwrap().file_name().into_string().unwrap();
                file.strip_suffix(".xcf").map(str::to_string)
            })
            .collect();
        names.sort();
        names
    }

    fn node_count(nodes: &[super::Node]) -> usize {
        nodes
            .iter()
            .map(|node| 1 + node_count(&node.children))
            .sum()
    }

    fn nested_groups(levels: usize, copies: usize) -> Vec<super::LayerInfo> {
        let bytes = std::fs::read(fixtures().join("gimp-group.xcf")).unwrap();
        let reader = XcfReader::from_bytes(&bytes).unwrap();
        let group = reader
            .document()
            .layers
            .iter()
            .find(|layer| layer.role == super::LayerRole::Group)
            .expect("a group")
            .clone();
        let mut layers = Vec::new();
        for level in 0..levels {
            for _ in 0..copies {
                let mut copy = group.clone();
                copy.path = vec![0; level + 1];
                layers.push(copy);
            }
        }
        layers
    }

    #[test]
    fn groups_sharing_one_path_cannot_multiply_the_tree() {
        let layers = nested_groups(15, 3);
        let tree = super::build_tree(&layers).expect("a tree");
        assert!(node_count(&tree) <= layers.len());
    }

    #[test]
    fn groups_nested_absurdly_deep_are_refused() {
        let layers = nested_groups(40, 1);
        assert!(matches!(
            super::build_tree(&layers),
            Err(super::XcfError::TooLarge)
        ));
    }

    #[test]
    fn every_blend_mode_and_scene_matches_what_gimp_draws() {
        let names = composed_names();
        assert!(names.len() > 90, "only {} cases found", names.len());
        for name in names {
            if name != "mode_color_erase" {
                assert_same_as_gimp(&composed(&name), &name);
            }
        }
    }

    #[test]
    fn the_first_real_images_match_what_gimp_draws() {
        for name in FIRST_FIXTURES {
            assert_same_as_gimp(
                &fixtures().join(format!("gimp-{name}.xcf")),
                &format!("first_{name}"),
            );
        }
    }

    #[test]
    fn a_mode_that_is_not_reproduced_is_reported_as_approximate() {
        let approximate = |name: &str| {
            let bytes = std::fs::read(composed(name)).unwrap();
            let mut reader = XcfReader::from_bytes(&bytes).unwrap();
            compose_image(&mut reader).unwrap().approximate
        };
        assert!(approximate("mode_color_erase"));
        assert!(!approximate("mode_multiply"));
        assert!(!approximate("s_nested"));
        assert!(!approximate("mode_erase"));
    }

    #[test]
    fn a_cut_or_damaged_image_never_panics_when_composed() {
        for name in ["s_group_normal", "s_layer_mask", "mode_hsv_hue"] {
            let bytes = std::fs::read(composed(name)).unwrap();
            for length in (0..bytes.len()).step_by(7) {
                if let Ok(mut reader) = XcfReader::from_bytes(&bytes[..length]) {
                    let _ = compose_image(&mut reader);
                }
            }
            for position in (IMAGE_HEADER_LENGTH..bytes.len()).step_by(3) {
                let mut damaged = bytes.clone();
                damaged[position] ^= 0xFF;
                if let Ok(mut reader) = XcfReader::from_bytes(&damaged) {
                    let _ = compose_image(&mut reader);
                }
            }
        }
    }
}
