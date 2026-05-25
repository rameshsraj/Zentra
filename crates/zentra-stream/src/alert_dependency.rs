//! Alert dependency tree for storm suppression.
//!
//! When a parent alert is FIRING, all registered child alerts are suppressed
//! automatically (their notifications are not sent, though state is still tracked).
//! This prevents alert storms — e.g., network down → suppress all host-down alerts.

use std::collections::{HashMap, HashSet};
use parking_lot::RwLock;
use uuid::Uuid;
use tracing::{debug, info};

/// A dependency relationship between two alert rules.
#[derive(Debug, Clone)]
pub struct AlertDependency {
    pub id: Uuid,
    pub parent_rule_id: Uuid,
    pub child_rule_id: Uuid,
    /// If > 0, child is suppressed only for this many minutes after parent fires.
    /// If 0, child is suppressed as long as parent is firing.
    pub suppress_duration_minutes: i64,
}

/// In-memory dependency registry.
/// Backed by the database; this is the fast lookup cache loaded at startup.
pub struct DependencyRegistry {
    /// parent_rule_id → set of child AlertDependency
    parent_to_children: RwLock<HashMap<Uuid, Vec<AlertDependency>>>,
    /// child_rule_id → set of parent_rule_ids
    child_to_parents: RwLock<HashMap<Uuid, HashSet<Uuid>>>,
}

impl DependencyRegistry {
    pub fn new() -> Self {
        Self {
            parent_to_children: RwLock::new(HashMap::new()),
            child_to_parents: RwLock::new(HashMap::new()),
        }
    }

    pub fn load(&self, deps: Vec<AlertDependency>) {
        let mut p2c = self.parent_to_children.write();
        let mut c2p = self.child_to_parents.write();
        p2c.clear();
        c2p.clear();
        for dep in deps {
            p2c.entry(dep.parent_rule_id).or_default().push(dep.clone());
            c2p.entry(dep.child_rule_id).or_default().insert(dep.parent_rule_id);
        }
        info!("Dependency registry loaded: {} relationships", p2c.values().map(|v| v.len()).sum::<usize>());
    }

    pub fn add(&self, dep: AlertDependency) {
        let mut p2c = self.parent_to_children.write();
        let mut c2p = self.child_to_parents.write();
        c2p.entry(dep.child_rule_id).or_default().insert(dep.parent_rule_id);
        p2c.entry(dep.parent_rule_id).or_default().push(dep);
    }

    pub fn remove(&self, dep_id: Uuid) {
        let mut p2c = self.parent_to_children.write();
        let mut c2p = self.child_to_parents.write();
        for children in p2c.values_mut() {
            if let Some(pos) = children.iter().position(|d| d.id == dep_id) {
                let removed = children.remove(pos);
                if let Some(parents) = c2p.get_mut(&removed.child_rule_id) {
                    parents.remove(&removed.parent_rule_id);
                }
                break;
            }
        }
    }

    /// Returns true if the given child_rule_id should be suppressed
    /// based on the currently firing parent rule IDs.
    pub fn is_suppressed(&self, child_rule_id: Uuid, firing_rule_ids: &HashSet<Uuid>) -> bool {
        let c2p = self.child_to_parents.read();
        if let Some(parents) = c2p.get(&child_rule_id) {
            let suppressed = parents.iter().any(|p| firing_rule_ids.contains(p));
            if suppressed {
                debug!(child = %child_rule_id, "Alert suppressed by dependency");
            }
            suppressed
        } else {
            false
        }
    }

    /// Returns all child rule IDs that should be suppressed when the given parent fires.
    pub fn children_of(&self, parent_rule_id: Uuid) -> Vec<Uuid> {
        let p2c = self.parent_to_children.read();
        p2c.get(&parent_rule_id)
            .map(|deps| deps.iter().map(|d| d.child_rule_id).collect())
            .unwrap_or_default()
    }

    /// Cycle detection: returns true if adding parent→child would create a cycle.
    pub fn would_create_cycle(&self, parent_id: Uuid, child_id: Uuid) -> bool {
        // DFS from child — if we reach parent_id, it's a cycle
        let p2c = self.parent_to_children.read();
        let mut visited = HashSet::new();
        let mut stack = vec![child_id];
        while let Some(node) = stack.pop() {
            if node == parent_id { return true; }
            if visited.insert(node) {
                if let Some(children) = p2c.get(&node) {
                    stack.extend(children.iter().map(|d| d.child_rule_id));
                }
            }
        }
        false
    }
}

impl Default for DependencyRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_suppression() {
        let registry = DependencyRegistry::new();
        let parent = Uuid::new_v4();
        let child = Uuid::new_v4();

        registry.add(AlertDependency {
            id: Uuid::new_v4(),
            parent_rule_id: parent,
            child_rule_id: child,
            suppress_duration_minutes: 0,
        });

        let mut firing = HashSet::new();
        assert!(!registry.is_suppressed(child, &firing));

        firing.insert(parent);
        assert!(registry.is_suppressed(child, &firing));
    }

    #[test]
    fn test_cycle_detection() {
        let registry = DependencyRegistry::new();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let c = Uuid::new_v4();

        registry.add(AlertDependency { id: Uuid::new_v4(), parent_rule_id: a, child_rule_id: b, suppress_duration_minutes: 0 });
        registry.add(AlertDependency { id: Uuid::new_v4(), parent_rule_id: b, child_rule_id: c, suppress_duration_minutes: 0 });

        // Adding c→a would create a cycle a→b→c→a
        assert!(registry.would_create_cycle(c, a));
        // Adding a→c directly is fine (already exists indirectly but no new cycle from fresh edge)
        // d has no deps — no cycle
        let d = Uuid::new_v4();
        assert!(!registry.would_create_cycle(d, a));
    }
}
