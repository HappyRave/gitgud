use serde::Serialize;

use super::Topology;

/// Lanes drawn before the rest fold into an edge marker (spike 4.3).
pub const MAX_LANES: usize = 32;
const CHECKPOINT_EVERY: usize = 256;

/// How one graph row is drawn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RowLayout {
    /// Lane of the commit dot.
    pub node: u16,
    pub segments: Vec<Segment>,
    /// Lanes beyond [`MAX_LANES`] were active and are not drawn.
    pub overflow: bool,
}

/// A line within one row, between lane positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Segment {
    pub from: u16,
    pub to: u16,
    pub part: Part,
    #[serde(skip_serializing_if = "Stroke::is_solid")]
    pub stroke: Stroke,
}

/// How a line is drawn: history plan previews dash the planned result and fade
/// the commits it replaces.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Stroke {
    #[default]
    Solid,
    Dashed,
    Faded,
}

impl Stroke {
    fn is_solid(&self) -> bool {
        *self == Stroke::Solid
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Part {
    /// Row top at `from` to the dot at `to`: a child's line arriving.
    Top,
    /// The dot at `from` to row bottom at `to`: a line leaving for a parent.
    Bottom,
    /// Row top to row bottom on lane `from` (= `to`): a line passing by.
    Through,
}

/// Each lane holds the row it is waiting for and how its line is drawn, `None` when free.
type Lanes = Vec<Option<(u32, Stroke)>>;

/// Lane state every [`CHECKPOINT_EVERY`] rows, so any window of rows is laid out
/// by replaying at most that many rows instead of keeping every row's segments.
pub struct Layout {
    checkpoints: Vec<Lanes>,
}

impl Layout {
    pub fn build(topology: &Topology) -> Self {
        let mut lanes = Lanes::new();
        let mut checkpoints = Vec::with_capacity(topology.len() / CHECKPOINT_EVERY + 1);
        for row in 0..topology.len() {
            if row % CHECKPOINT_EVERY == 0 {
                checkpoints.push(lanes.clone());
            }
            step(&mut lanes, row, topology.parents(row), topology.stroke(row));
        }
        Self { checkpoints }
    }

    /// Layouts for rows `start..start + count`, clamped to the topology.
    pub fn rows(&self, topology: &Topology, start: usize, count: usize) -> Vec<RowLayout> {
        let end = start.saturating_add(count).min(topology.len());
        if start >= end {
            return Vec::new();
        }
        let first = start - start % CHECKPOINT_EVERY;
        let mut lanes = self.checkpoints[first / CHECKPOINT_EVERY].clone();
        for row in first..start {
            step(&mut lanes, row, topology.parents(row), topology.stroke(row));
        }
        (start..end)
            .map(|row| {
                cap(step(
                    &mut lanes,
                    row,
                    topology.parents(row),
                    topology.stroke(row),
                ))
            })
            .collect()
    }
}

/// Lays out `row` and advances `lanes` past it. `stroke` is how the lines
/// leaving the row are drawn, all the way down to its parents.
fn step(lanes: &mut Lanes, row: usize, parents: &[u32], stroke: Stroke) -> RowLayout {
    let me = u32::try_from(row).expect("row fits u32");
    let waits_for = |l: &Option<(u32, Stroke)>, r: u32| l.is_some_and(|(w, _)| w == r);
    let node = match lanes.iter().position(|l| waits_for(l, me)) {
        Some(lane) => lane,
        None => free_lane(lanes),
    };

    let mut segments = Vec::with_capacity(lanes.len() + parents.len());
    for (lane, waiting) in lanes.iter_mut().enumerate() {
        match *waiting {
            Some((w, s)) if w == me => {
                segments.push(seg(lane, node, Part::Top, s));
                *waiting = None;
            }
            Some((_, s)) => segments.push(seg(lane, lane, Part::Through, s)),
            None => {}
        }
    }

    for (i, &parent) in parents.iter().enumerate() {
        let lane = if i == 0 {
            node
        } else if let Some(lane) = lanes.iter().position(|l| waits_for(l, parent)) {
            lane
        } else {
            free_lane(lanes)
        };
        lanes[lane] = Some((parent, stroke));
        segments.push(seg(node, lane, Part::Bottom, stroke));
    }

    while lanes.last() == Some(&None) {
        lanes.pop();
    }
    RowLayout {
        node: lane_u16(node),
        segments,
        overflow: false,
    }
}

fn free_lane(lanes: &mut Lanes) -> usize {
    lanes.iter().position(Option::is_none).unwrap_or_else(|| {
        lanes.push(None);
        lanes.len() - 1
    })
}

fn seg(from: usize, to: usize, part: Part, stroke: Stroke) -> Segment {
    Segment {
        from: lane_u16(from),
        to: lane_u16(to),
        part,
        stroke,
    }
}

fn lane_u16(lane: usize) -> u16 {
    u16::try_from(lane).expect("more than u16::MAX lanes")
}

fn cap(mut row: RowLayout) -> RowLayout {
    let max = lane_u16(MAX_LANES);
    let before = row.segments.len();
    row.segments.retain(|s| s.from < max && s.to < max);
    row.overflow = row.segments.len() != before || row.node >= max;
    row.node = row.node.min(max - 1);
    row
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn t(parents: &[&[u32]]) -> Topology {
        Topology::from_parents(parents)
    }

    fn all(topology: &Topology) -> Vec<RowLayout> {
        Layout::build(topology).rows(topology, 0, topology.len())
    }

    fn top(from: u16, to: u16) -> Segment {
        seg(from.into(), to.into(), Part::Top, Stroke::Solid)
    }
    fn bottom(from: u16, to: u16) -> Segment {
        seg(from.into(), to.into(), Part::Bottom, Stroke::Solid)
    }
    fn through(lane: u16) -> Segment {
        seg(lane.into(), lane.into(), Part::Through, Stroke::Solid)
    }

    #[test]
    fn a_rows_stroke_follows_its_lines_down_to_the_parent() {
        // 0 ghost(2) / 1 tip(2), faded / 2 base
        let mut topology = t(&[&[2], &[2], &[]]);
        topology.set_stroke(0, Stroke::Dashed);
        topology.set_stroke(1, Stroke::Faded);
        let rows = all(&topology);
        let strokes = |r: &RowLayout| r.segments.iter().map(|s| s.stroke).collect::<Vec<_>>();
        assert_eq!(strokes(&rows[0]), [Stroke::Dashed]);
        assert_eq!(strokes(&rows[1]), [Stroke::Dashed, Stroke::Faded]);
        assert_eq!(strokes(&rows[2]), [Stroke::Dashed, Stroke::Faded]);
    }
    fn row(node: u16, segments: Vec<Segment>) -> RowLayout {
        RowLayout {
            node,
            segments,
            overflow: false,
        }
    }

    #[test]
    fn linear_history_is_one_straight_lane() {
        assert_eq!(
            all(&t(&[&[1], &[2], &[]])),
            [
                row(0, vec![bottom(0, 0)]),
                row(0, vec![top(0, 0), bottom(0, 0)]),
                row(0, vec![top(0, 0)]),
            ]
        );
    }

    #[test]
    fn branch_and_merge() {
        // 0 merge(3, 1) / 1 f2 / 2 f1 / 3 m1 / 4 base: git's topo order for a --no-ff merge.
        assert_eq!(
            all(&t(&[&[3, 1], &[2], &[4], &[4], &[]])),
            [
                row(0, vec![bottom(0, 0), bottom(0, 1)]),
                row(1, vec![through(0), top(1, 1), bottom(1, 1)]),
                row(1, vec![through(0), top(1, 1), bottom(1, 1)]),
                row(0, vec![top(0, 0), through(1), bottom(0, 0)]),
                row(0, vec![top(0, 0), top(1, 0)]),
            ]
        );
    }

    #[test]
    fn second_parent_joins_a_lane_already_waiting_for_it() {
        // 0 tip(2) / 1 merge(3, 2) / 2 a(3) / 3 base
        let rows = all(&t(&[&[2], &[3, 2], &[3], &[]]));
        assert_eq!(
            rows[1],
            row(1, vec![through(0), bottom(1, 1), bottom(1, 0)])
        );
        assert_eq!(rows[2], row(0, vec![top(0, 0), through(1), bottom(0, 0)]));
    }

    #[test]
    fn octopus_fans_out_to_new_lanes() {
        let rows = all(&t(&[&[1, 2, 3], &[4], &[4], &[4], &[]]));
        assert_eq!(
            rows[0],
            row(0, vec![bottom(0, 0), bottom(0, 1), bottom(0, 2)])
        );
        assert_eq!(rows[4], row(0, vec![top(0, 0), top(1, 0), top(2, 0)]));
    }

    #[test]
    fn a_new_tip_takes_the_first_free_lane() {
        // 0 a(2) / 1 b(3), a second tip / 2 c(3) / 3 base
        let rows = all(&t(&[&[2], &[3], &[3], &[]]));
        assert_eq!(rows[1], row(1, vec![through(0), bottom(1, 1)]));
    }

    #[test]
    fn freed_lanes_are_reused_and_trailing_ones_trimmed() {
        // Two separate roots: the second history reuses lane 0.
        let rows = all(&t(&[&[1], &[], &[3], &[]]));
        assert_eq!(rows[2], row(0, vec![bottom(0, 0)]));
    }

    #[test]
    fn lanes_beyond_the_cap_fold_into_an_overflow_marker() {
        let n = MAX_LANES as u32 + 4;
        // Row 0 is an octopus with n parents, each parent a root.
        let fan: Vec<u32> = (1..=n).collect();
        let mut parents: Vec<&[u32]> = vec![&fan];
        parents.extend(std::iter::repeat_n(&[][..], n as usize));
        let topology = t(&parents);
        let rows = all(&topology);
        assert!(rows[0].overflow);
        assert_eq!(rows[0].segments.len(), MAX_LANES);
        let last = rows.last().unwrap();
        assert_eq!(last.node, MAX_LANES as u16 - 1);
        assert!(last.overflow);
        let max = MAX_LANES as u16;
        assert!(
            rows.iter()
                .flat_map(|r| &r.segments)
                .all(|s| s.from < max && s.to < max)
        );
    }

    #[test]
    fn empty_and_out_of_range_windows() {
        let topology = t(&[&[1], &[]]);
        let layout = Layout::build(&topology);
        assert!(layout.rows(&topology, 2, 10).is_empty());
        assert!(layout.rows(&topology, 0, 0).is_empty());
        assert_eq!(layout.rows(&topology, 1, usize::MAX).len(), 1);
        assert!(Layout::build(&t(&[])).rows(&t(&[]), 0, 10).is_empty());
    }

    /// Random DAGs: each row gets up to 3 distinct parents among later rows.
    fn dag() -> impl Strategy<Value = Vec<Vec<u32>>> {
        prop::collection::vec(prop::collection::vec(any::<u16>(), 0..4), 1..700).prop_map(|raw| {
            let n = raw.len();
            raw.into_iter()
                .enumerate()
                .map(|(row, picks)| {
                    let later = n - row - 1;
                    let mut ps: Vec<u32> = Vec::new();
                    if later > 0 {
                        for pick in picks {
                            // Mostly near parents, like real history.
                            let offset = (pick as usize % later.min(8 + pick as usize % 64)) + 1;
                            let p = (row + offset.min(later)) as u32;
                            if !ps.contains(&p) {
                                ps.push(p);
                            }
                        }
                    }
                    ps
                })
                .collect()
        })
    }

    fn topology_of(parents: &[Vec<u32>]) -> Topology {
        let slices: Vec<&[u32]> = parents.iter().map(Vec::as_slice).collect();
        Topology::from_parents(&slices)
    }

    proptest! {
        #[test]
        fn any_window_equals_the_full_layout(parents in dag(), start in 0usize..700, count in 0usize..600) {
            let topology = topology_of(&parents);
            let layout = Layout::build(&topology);
            let full = layout.rows(&topology, 0, topology.len());
            let window = layout.rows(&topology, start, count);
            let end = (start + count).min(topology.len());
            let expected = if start < end { &full[start..end] } else { &full[0..0] };
            prop_assert_eq!(window.as_slice(), expected);
        }

        #[test]
        fn every_edge_reaches_its_parent(parents in dag()) {
            let topology = topology_of(&parents);
            let rows = all(&topology);
            let max = MAX_LANES as u16;
            for (child, ps) in parents.iter().enumerate() {
                if rows[child].overflow {
                    continue;
                }
                let bottoms: Vec<u16> = rows[child].segments.iter()
                    .filter(|s| s.part == Part::Bottom).map(|s| s.to).collect();
                prop_assert_eq!(bottoms.len(), ps.len());
                for (&parent, &lane) in ps.iter().zip(&bottoms) {
                    let mut r = child + 1;
                    loop {
                        prop_assert!(r <= parent as usize, "lane {} from row {} overshot parent {}", lane, child, parent);
                        let segs = &rows[r].segments;
                        if segs.iter().any(|s| s.part == Part::Through && s.from == lane) {
                            r += 1;
                            continue;
                        }
                        prop_assert!(lane < max);
                        let arrives = segs.iter().any(|s| s.part == Part::Top && s.from == lane && s.to == rows[r].node);
                        prop_assert!(arrives, "lane {} from row {} broke at row {}", lane, child, r);
                        prop_assert_eq!(r, parent as usize);
                        break;
                    }
                }
            }
        }

        // Lines may converge at a row's bottom edge (a merge), but each lane
        // carries at most one line down from the row above, and none passes
        // through the dot. Overflow rows fold far dots onto the edge lane.
        #[test]
        fn each_lane_enters_a_row_once(parents in dag()) {
            let topology = topology_of(&parents);
            for row in all(&topology).into_iter().filter(|r| !r.overflow) {
                let mut upper: Vec<u16> = row.segments.iter()
                    .filter(|s| s.part != Part::Bottom).map(|s| s.from).collect();
                let len = upper.len();
                upper.sort_unstable();
                upper.dedup();
                prop_assert_eq!(upper.len(), len);
                prop_assert!(row.segments.iter().all(|s| s.part != Part::Through || s.from != row.node));
            }
        }

        #[test]
        fn lanes_leaving_a_row_enter_the_next(parents in dag()) {
            let topology = topology_of(&parents);
            let rows = all(&topology);
            for pair in rows.windows(2) {
                if pair[0].overflow || pair[1].overflow {
                    continue;
                }
                let mut leaving: Vec<u16> = pair[0].segments.iter()
                    .filter(|s| s.part != Part::Top).map(|s| s.to).collect();
                let mut entering: Vec<u16> = pair[1].segments.iter()
                    .filter(|s| s.part != Part::Bottom).map(|s| s.from).collect();
                leaving.sort_unstable();
                leaving.dedup();
                entering.sort_unstable();
                prop_assert_eq!(leaving, entering);
            }
        }
    }
}
