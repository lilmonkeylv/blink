use serde::Serialize;

use crate::log::CommitSummary;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Edge {
    pub from: u16,
    pub to: u16,
    pub color: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphRow {
    pub lane: u16,
    pub color: u16,
    pub top: Vec<Edge>,
    pub bottom: Vec<Edge>,
    pub width: u16,
}

#[derive(Debug, Clone, Default)]
pub struct GraphState {
    lanes: Vec<Option<(String, u16)>>,
    next_color: u16,
}

impl GraphState {
    fn alloc(&mut self) -> usize {
        match self.lanes.iter().position(Option::is_none) {
            Some(i) => i,
            None => {
                self.lanes.push(None);
                self.lanes.len() - 1
            }
        }
    }

    fn color(&mut self) -> u16 {
        let c = self.next_color;
        self.next_color = self.next_color.wrapping_add(1);
        c
    }

    pub fn push(&mut self, commit: &CommitSummary) -> GraphRow {
        let before = self.lanes.clone();
        let lane = match before.iter().position(|s| s.as_ref().is_some_and(|(id, _)| *id == commit.id)) {
            Some(i) => i,
            None => {
                let i = self.alloc();
                let c = self.color();
                self.lanes[i] = Some((commit.id.clone(), c));
                i
            }
        };
        let color = self.lanes[lane].as_ref().map(|(_, c)| *c).unwrap_or_default();

        let mut top = Vec::new();
        for (i, slot) in before.iter().enumerate() {
            if let Some((id, c)) = slot {
                let to = if *id == commit.id { lane } else { i };
                top.push(Edge { from: i as u16, to: to as u16, color: *c });
                if *id == commit.id && i != lane {
                    self.lanes[i] = None;
                }
            }
        }

        let mut spawned = Vec::new();
        let mut folded: Option<usize> = None;
        match commit.parents.first() {
            Some(first) => {
                let existing = self
                    .lanes
                    .iter()
                    .enumerate()
                    .position(|(i, s)| i != lane && s.as_ref().is_some_and(|(id, _)| id == first));
                match existing {
                    Some(target) if target > lane => {
                        self.lanes[target] = None;
                        self.lanes[lane] = Some((first.clone(), color));
                        folded = Some(target);
                    }
                    Some(target) => {
                        spawned.push((target, self.lanes[target].as_ref().unwrap().1));
                        self.lanes[lane] = None;
                    }
                    None => self.lanes[lane] = Some((first.clone(), color)),
                }
            }
            None => self.lanes[lane] = None,
        }
        for parent in commit.parents.iter().skip(1) {
            let target = match self.lanes.iter().position(|s| s.as_ref().is_some_and(|(id, _)| id == parent)) {
                Some(i) => i,
                None => {
                    let i = self.alloc();
                    let c = self.color();
                    self.lanes[i] = Some((parent.clone(), c));
                    i
                }
            };
            spawned.push((target, self.lanes[target].as_ref().unwrap().1));
        }

        let mut bottom = Vec::new();
        for (i, slot) in before.iter().enumerate() {
            if let Some((id, c)) = slot
                && i != lane
                && *id != commit.id
            {
                let to = if folded == Some(i) { lane } else { i };
                bottom.push(Edge { from: i as u16, to: to as u16, color: *c });
            }
        }
        if let Some((_, c)) = &self.lanes[lane] {
            bottom.push(Edge { from: lane as u16, to: lane as u16, color: *c });
        }
        for (target, c) in spawned {
            if target != lane && !bottom.iter().any(|e| e.from as usize == lane && e.to as usize == target) {
                bottom.push(Edge { from: lane as u16, to: target as u16, color: c });
            }
        }

        while self.lanes.last().is_some_and(Option::is_none) {
            self.lanes.pop();
        }
        let width = top.iter().chain(&bottom).map(|e| e.from.max(e.to) + 1).max().unwrap_or(1).max(lane as u16 + 1);
        GraphRow { lane: lane as u16, color, top, bottom, width }
    }
}

pub fn layout(commits: &[CommitSummary]) -> Vec<GraphRow> {
    let mut state = GraphState::default();
    commits.iter().map(|c| state.push(c)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(id: &str, parents: &[&str]) -> CommitSummary {
        CommitSummary {
            id: id.into(),
            short_id: id.into(),
            summary: String::new(),
            author_name: String::new(),
            author_email: String::new(),
            time: 0,
            parents: parents.iter().map(|p| p.to_string()).collect(),
        }
    }

    #[test]
    fn linear_history_stays_in_lane_zero() {
        let rows = layout(&[c("c", &["b"]), c("b", &["a"]), c("a", &[])]);
        assert!(rows.iter().all(|r| r.lane == 0 && r.width == 1));
        assert!(rows[2].bottom.is_empty());
    }

    #[test]
    fn merge_opens_and_closes_a_lane() {
        let rows = layout(&[c("m", &["a", "f"]), c("f", &["base"]), c("a", &["base"]), c("base", &[])]);
        assert_eq!(rows[0].lane, 0);
        assert!(rows[0].bottom.iter().any(|e| e.from == 0 && e.to == 1));
        assert_eq!(rows[1].lane, 1);
        assert_eq!(rows[2].lane, 0);
        assert_eq!(rows[3].lane, 0);
        assert!(rows[2].bottom.iter().any(|e| e.from == 1 && e.to == 0));
        assert_eq!(rows[3].width, 1);
    }

    #[test]
    fn two_tips_get_separate_lanes() {
        let rows = layout(&[c("x", &["base"]), c("y", &["base"]), c("base", &[])]);
        assert_eq!(rows[0].lane, 0);
        assert_eq!(rows[1].lane, 1);
        assert_eq!(rows[2].lane, 0);
    }
}
