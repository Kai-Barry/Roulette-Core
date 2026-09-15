//! Branching node map generation (§2.3, TASK-037).

use crate::rng::Rng;
use serde::{Deserialize, Serialize};

/// Encounter kinds (§2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    Combat,
    Elite,
    Shop,
    Event,
    Forge,
    Boss,
}

impl NodeType {
    /// Icon per §2.4 (combat 💀 / elite 👹 / shop ⚡ / event ❓ / forge 🔥 / boss 👑).
    pub fn icon(self) -> &'static str {
        match self {
            NodeType::Combat => "💀",
            NodeType::Elite => "👹",
            NodeType::Shop => "⚡",
            NodeType::Event => "❓",
            NodeType::Forge => "🔥",
            NodeType::Boss => "👑",
        }
    }
}

/// One map node (§2.3 `MapNode`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MapNode {
    /// Stable id: `f{floor}l{lane}` (lanes are unique within a floor).
    pub id: String,
    pub node_type: NodeType,
    pub floor: usize,
    pub lane: u8,
    /// Ids of nodes reachable from this node.
    pub connections: Vec<String>,
    pub completed: bool,
}

impl MapNode {
    pub fn new(id: String, node_type: NodeType, floor: usize, lane: u8) -> Self {
        Self { id, node_type, floor, lane, connections: Vec::new(), completed: false }
    }
}

/// Floors of nodes; `floors[i]` is the row of nodes on floor `i`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Map {
    pub floors: Vec<Vec<MapNode>>,
}

impl Map {
    pub fn node(&self, id: &str) -> Option<&MapNode> {
        self.floors.iter().flatten().find(|n| n.id == id)
    }

    pub fn node_mut(&mut self, id: &str) -> Option<&mut MapNode> {
        self.floors.iter_mut().flatten().find(|n| n.id == id)
    }

    /// Final floor index (the boss floor).
    pub fn last_floor(&self) -> usize {
        self.floors.len().saturating_sub(1)
    }
}

/// Generates the branching map exactly per §2.3.
pub fn generate(floors: usize, rng: &mut Rng) -> Map {
    let mut map = Map::default();
    if floors == 0 {
        return map;
    }

    // Floor 0: exactly 3 combat nodes, one per lane.
    map.floors.push(vec![
        MapNode::new("f0l0".into(), NodeType::Combat, 0, 0),
        MapNode::new("f0l1".into(), NodeType::Combat, 0, 1),
        MapNode::new("f0l2".into(), NodeType::Combat, 0, 2),
    ]);

    // Middle floors: 2–3 nodes with the ordered type roll.
    let last = floors - 1;
    for f in 1..floors {
        if f == last {
            map.floors.push(vec![MapNode::new(format!("f{f}l1"), NodeType::Boss, f, 1)]);
            break;
        }
        // 2–3 nodes; lanes are distinct picks from {0,1,2}.
        let count = 2 + (rng.next_u32() % 2) as usize;
        let mut lanes = [0u8, 1, 2];
        rng.shuffle(&mut lanes);
        lanes[..count].sort_unstable();
        let row = lanes[..count]
            .iter()
            .map(|&lane| {
                let node_type = roll_node_type(f, rng);
                MapNode::new(format!("f{f}l{lane}"), node_type, f, lane)
            })
            .collect();
        map.floors.push(row);
    }

    connect(&mut map, rng);
    fix_orphans(&mut map);
    map
}

/// Type roll in §2.3 order: floor%4==3 → elite; floor%4==1 && floor>1 → shop;
/// else rand <0.2 shop, <0.4 event, <0.55 forge, else combat.
fn roll_node_type(floor: usize, rng: &mut Rng) -> NodeType {
    if floor % 4 == 3 {
        return NodeType::Elite;
    }
    if floor % 4 == 1 && floor > 1 {
        return NodeType::Shop;
    }
    let roll = rng.next_f64();
    if roll < 0.2 {
        NodeType::Shop
    } else if roll < 0.4 {
        NodeType::Event
    } else if roll < 0.55 {
        NodeType::Forge
    } else {
        NodeType::Combat
    }
}

/// Wires connections: each node connects to the closest-lane node on the next
/// floor; a second connection to the next-closest lane joins with 40%
/// probability when it is ≤ 1 lane away.
fn connect(map: &mut Map, rng: &mut Rng) {
    for f in 0..map.floors.len().saturating_sub(1) {
        let next: Vec<(String, u8)> =
            map.floors[f + 1].iter().map(|n| (n.id.clone(), n.lane)).collect();
        if next.is_empty() {
            continue;
        }
        for node in &mut map.floors[f] {
            let mut ranked = next.clone();
            ranked.sort_by_key(|&(_, lane)| (lane.abs_diff(node.lane), lane));
            node.connections.push(ranked[0].0.clone());
            if let Some(&(_, lane)) = ranked.get(1) {
                if lane.abs_diff(node.lane) <= 1 && rng.next_f64() < 0.4 {
                    node.connections.push(ranked[1].0.clone());
                }
            }
        }
    }
}

/// Any next-floor node with zero incoming connections gets one from the
/// closest-lane node of the previous floor (guarantees reachability).
fn fix_orphans(map: &mut Map) {
    for f in 1..map.floors.len() {
        let incoming: Vec<String> =
            map.floors[f - 1].iter().flat_map(|n| n.connections.iter().cloned()).collect();
        let prev: Vec<(String, u8)> =
            map.floors[f - 1].iter().map(|n| (n.id.clone(), n.lane)).collect();
        for node_idx in 0..map.floors[f].len() {
            let node = &map.floors[f][node_idx];
            let orphan = !incoming.contains(&node.id);
            let node_id = node.id.clone();
            if orphan {
                let mut ranked = prev.clone();
                ranked.sort_by_key(|&(_, lane)| (lane.abs_diff(node.lane), lane));
                let donor = ranked.first().map(|(id, _)| id.clone());
                if let Some(id) = donor {
                    if let Some(prev_node) = map.node_mut(&id) {
                        prev_node.connections.push(node_id);
                    }
                }
            }
        }
    }
}
