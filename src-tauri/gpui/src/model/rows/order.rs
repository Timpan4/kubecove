//! Display order from rows in sort order: ownership groups, then kind lanes,
//! as `buildResourceTableDisplay` and `buildResourceTableEntries` group them.
//! Rows are counted per lane, then placed, so no lane holds its own list.

use super::PreparedRows;

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
struct OrderKey {
    group_rank: u8,
    group_first: usize,
    lane_rank: usize,
    lane_first: usize,
}

/// The first row of a group or lane: preferred rows lead when grouped, then
/// the row's position in the sorted sequence.
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
struct First {
    ordinary: bool,
    position: usize,
}

impl First {
    const UNSEEN: Self = Self {
        ordinary: true,
        position: usize::MAX,
    };
}

/// What grouping reads besides the rows: whether rows group by GitOps owner,
/// which rows are preferred, and each kind's lane position.
pub(super) struct Lanes<'a> {
    pub source: &'a PreparedRows,
    pub grouped: bool,
    pub preferred: &'a [bool],
    pub lane_order: &'a [usize],
}

impl Lanes<'_> {
    /// The row's group and lane. Ungrouped rows share group 0 and lane by kind.
    fn of(&self, row: usize) -> (usize, usize, bool) {
        let key = &self.source.keys[row];
        if self.grouped {
            (key.group, key.lane, self.preferred[row])
        } else {
            (0, key.kind, false)
        }
    }
}

/// Reused buffers for [`Grouping::order`].
#[derive(Default)]
pub(super) struct Grouping {
    group_first: Vec<First>,
    lane_first: Vec<First>,
    lanes: Vec<(OrderKey, usize)>,
    /// Per lane: preferred and ordinary row counts, then their next slots.
    slots: Vec<[usize; 2]>,
}

impl Grouping {
    /// Writes `sequence`, rows in sort order, to `out` in display order. A
    /// group or lane takes the place of its first row; preferred rows lead
    /// their group and lane when rows group by owner.
    pub(super) fn order(&mut self, lanes: &Lanes<'_>, sequence: &[usize], out: &mut Vec<usize>) {
        let source = lanes.source;
        let lane_count = if lanes.grouped {
            source.lanes.len()
        } else {
            source.kinds.len()
        };
        self.group_first.clear();
        self.group_first.resize(source.groups.len(), First::UNSEEN);
        self.lane_first.clear();
        self.lane_first.resize(lane_count, First::UNSEEN);
        self.slots.clear();
        self.slots.resize(lane_count, [0; 2]);
        for (position, &row) in sequence.iter().enumerate() {
            let (group, lane, preferred) = lanes.of(row);
            let first = First {
                ordinary: !preferred,
                position,
            };
            self.group_first[group] = self.group_first[group].min(first);
            self.lane_first[lane] = self.lane_first[lane].min(first);
            self.slots[lane][usize::from(!preferred)] += 1;
        }
        self.lanes.clear();
        for lane in 0..lane_count {
            if self.lane_first[lane] == First::UNSEEN {
                continue;
            }
            let (group, kind) = if lanes.grouped {
                let lane = &source.lanes[lane];
                (lane.group, lane.kind)
            } else {
                (0, lane)
            };
            self.lanes.push((
                OrderKey {
                    group_rank: if !lanes.grouped || !self.group_first[group].ordinary {
                        0
                    } else {
                        source.groups[group] + 1
                    },
                    group_first: self.group_first[group].position,
                    lane_rank: if lanes.grouped && !self.lane_first[lane].ordinary {
                        0
                    } else {
                        lanes.lane_order[kind] + 1
                    },
                    lane_first: self.lane_first[lane].position,
                },
                lane,
            ));
        }
        self.lanes.sort_unstable_by_key(|&(key, _)| key);
        let mut next = 0;
        for &(_, lane) in &self.lanes {
            let [preferred, ordinary] = self.slots[lane];
            self.slots[lane] = [next, next + preferred];
            next += preferred + ordinary;
        }
        out.clear();
        out.resize(sequence.len(), 0);
        for &row in sequence {
            let (_, lane, preferred) = lanes.of(row);
            let slot = &mut self.slots[lane][usize::from(!preferred)];
            out[*slot] = row;
            *slot += 1;
        }
    }
}
