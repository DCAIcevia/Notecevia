//! 对齐与等距分布：只改几何，不碰正文。

use crate::board::Document;

/// 对齐的参照边。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alignment {
    Left,
    CenterX,
    Right,
    Top,
    CenterY,
    Bottom,
}

/// 分布的方向。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

impl Axis {
    /// 取块在这条轴上的起点与长度。
    const fn span(self, geometry: &crate::board::Geometry) -> (f32, f32) {
        match self {
            Self::Horizontal => (geometry.x, geometry.width),
            Self::Vertical => (geometry.y, geometry.height),
        }
    }
}

impl Document {
    /// 这些块合起来的外接矩形：(x, y, 宽, 高)。空选择给 None。
    pub fn bounds(&self, ids: &[u64]) -> Option<(f32, f32, f32, f32)> {
        let mut bounds: Option<(f32, f32, f32, f32)> = None;
        for id in ids {
            let Some(entry) = self.entry(*id) else {
                continue;
            };
            let geometry = entry.geometry;
            bounds = Some(match bounds {
                None => (
                    geometry.x,
                    geometry.y,
                    geometry.x + geometry.width,
                    geometry.y + geometry.height,
                ),
                Some((min_x, min_y, max_x, max_y)) => (
                    min_x.min(geometry.x),
                    min_y.min(geometry.y),
                    max_x.max(geometry.x + geometry.width),
                    max_y.max(geometry.y + geometry.height),
                ),
            });
        }
        bounds.map(|(min_x, min_y, max_x, max_y)| (min_x, min_y, max_x - min_x, max_y - min_y))
    }

    /// 按整体外接矩形对齐选中的块。
    pub fn align(&mut self, ids: &[u64], alignment: Alignment) {
        let Some((x, y, width, height)) = self.bounds(ids) else {
            return;
        };
        for id in ids {
            let Some(entry) = self.entry_mut(*id) else {
                continue;
            };
            let geometry = &mut entry.geometry;
            match alignment {
                Alignment::Left => geometry.x = x,
                Alignment::CenterX => geometry.x = x + (width - geometry.width) / 2.0,
                Alignment::Right => geometry.x = x + width - geometry.width,
                Alignment::Top => geometry.y = y,
                Alignment::CenterY => geometry.y = y + (height - geometry.height) / 2.0,
                Alignment::Bottom => geometry.y = y + height - geometry.height,
            }
        }
    }

    /// 等距分布：两头的块不动，中间的块按间隙均分。少于三块没有可分布的中间项。
    pub fn distribute(&mut self, ids: &[u64], axis: Axis) {
        let mut items: Vec<(u64, f32, f32)> = ids
            .iter()
            .filter_map(|id| {
                let entry = self.entry(*id)?;
                let (start, size) = axis.span(&entry.geometry);
                Some((*id, start, size))
            })
            .collect();
        if items.len() < 3 {
            return;
        }
        items.sort_by(|left, right| left.1.total_cmp(&right.1));

        let first = items[0];
        let last = items[items.len() - 1];
        let span = (last.1 + last.2) - first.1;
        let occupied: f32 = items.iter().map(|(_, _, size)| size).sum();
        let gap = (span - occupied) / (items.len() as f32 - 1.0);

        let mut cursor = first.1;
        for (id, _, size) in items {
            if let Some(entry) = self.entry_mut(id) {
                match axis {
                    Axis::Horizontal => entry.geometry.x = cursor,
                    Axis::Vertical => entry.geometry.y = cursor,
                }
            }
            cursor += size + gap;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{BlockKind, Document, Entry, Geometry};

    fn document_with(geometries: &[(f32, f32, f32, f32)]) -> (Document, Vec<u64>) {
        let mut document = Document::new("画板");
        let ids = geometries
            .iter()
            .map(|(x, y, width, height)| {
                document.add_block(BlockKind::Text, Geometry::new(*x, *y, *width, *height))
            })
            .collect();
        (document, ids)
    }

    #[test]
    fn bounds_covers_every_block() {
        let (document, ids) = document_with(&[(0.0, 0.0, 10.0, 10.0), (20.0, 30.0, 10.0, 10.0)]);
        assert_eq!(document.bounds(&ids), Some((0.0, 0.0, 30.0, 40.0)));
    }

    #[test]
    fn align_left_puts_every_left_edge_on_the_bounds() {
        let (mut document, ids) =
            document_with(&[(10.0, 0.0, 10.0, 10.0), (50.0, 20.0, 10.0, 10.0)]);
        document.align(&ids, Alignment::Left);
        for id in &ids {
            assert_eq!(document.entry(*id).unwrap().geometry.x, 10.0);
        }
    }

    #[test]
    fn align_center_keeps_widths() {
        let (mut document, ids) = document_with(&[(0.0, 0.0, 10.0, 10.0), (0.0, 0.0, 30.0, 10.0)]);
        document.align(&ids, Alignment::CenterX);
        assert_eq!(document.entry(ids[0]).unwrap().geometry.x, 10.0);
        assert_eq!(document.entry(ids[1]).unwrap().geometry.x, 0.0);
    }

    #[test]
    fn distribute_equalises_gaps_and_keeps_ends() {
        let (mut document, ids) = document_with(&[
            (0.0, 0.0, 10.0, 10.0),
            (20.0, 0.0, 10.0, 10.0),
            (100.0, 0.0, 10.0, 10.0),
        ]);
        document.distribute(&ids, Axis::Horizontal);
        let starts: Vec<f32> = ids
            .iter()
            .map(|id| document.entry(*id).unwrap().geometry.x)
            .collect();
        assert_eq!(starts, vec![0.0, 50.0, 100.0]);
    }

    #[test]
    fn distribute_needs_three_blocks() {
        let (mut document, ids) = document_with(&[(0.0, 0.0, 10.0, 10.0), (20.0, 0.0, 10.0, 10.0)]);
        let before = document.clone();
        document.distribute(&ids, Axis::Horizontal);
        assert_eq!(document, before);
    }

    #[test]
    fn entries_with_unknown_ids_are_ignored() {
        let (mut document, _) = document_with(&[(0.0, 0.0, 10.0, 10.0)]);
        document.board.entries.push(Entry::new(
            99,
            BlockKind::Text,
            Geometry::new(5.0, 5.0, 10.0, 10.0),
        ));
        document.align(&[1, 42], Alignment::Left);
        assert_eq!(document.entry(1).unwrap().geometry.x, 0.0);
    }
}
