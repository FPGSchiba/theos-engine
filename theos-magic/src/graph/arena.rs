//! A hand-rolled generational arena (AC1).
//!
//! Nodes live in one contiguous store; a [`NodeId`] is a slot `index` plus a
//! `generation` stamp. Removing a node bumps its slot generation, so a stale
//! `NodeId` (carrying the old generation) fails lookup instead of silently
//! aliasing a value that has since been inserted into the reused slot — the ABA
//! guard the story's AC1 exercises.

/// A stable handle into an [`Arena`].
///
/// Carries the slot `index` and the `generation` that slot had when the handle was
/// minted. Lookup fails if the slot's current generation no longer matches.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct NodeId {
    index: u32,
    generation: u32,
}

impl NodeId {
    /// The slot index this handle refers to.
    pub fn index(self) -> u32 {
        self.index
    }

    /// The generation stamp this handle was minted with.
    pub fn generation(self) -> u32 {
        self.generation
    }
}

/// A generational arena storing values of type `T` in reusable slots.
#[derive(Clone, Debug)]
pub struct Arena<T> {
    generations: Vec<u32>,
    values: Vec<Option<T>>,
    free: Vec<u32>,
}

impl<T> Arena<T> {
    /// Create an empty arena.
    pub fn new() -> Self {
        Self {
            generations: Vec::new(),
            values: Vec::new(),
            free: Vec::new(),
        }
    }

    /// Insert `value`, returning a fresh handle. Reuses a freed slot when one is
    /// available, otherwise grows the store.
    pub fn insert(&mut self, value: T) -> NodeId {
        let free_slot = self.free.pop();
        if let Some(slot) = free_slot {
            self.values[slot as usize] = Some(value);
            NodeId {
                index: slot,
                generation: self.generations[slot as usize],
            }
        } else {
            let index = self.values.len();
            self.values.push(Some(value));
            self.generations.push(0);
            NodeId {
                index: index as u32,
                generation: 0,
            }
        }
    }

    /// Whether `id` is a currently-valid handle into this arena: its slot index is
    /// in range and the slot's live generation still matches the one the handle was
    /// minted with. A `true` result also implies the slot is occupied, since a
    /// freed slot's generation always runs ahead of any outstanding handle to it.
    fn is_valid_handle(&self, id: NodeId) -> bool {
        let slot = id.index() as usize;
        slot < self.values.len() && self.generations[slot] == id.generation()
    }

    /// Borrow the value at `id`, or `None` if the handle is stale (its generation
    /// no longer matches the slot's).
    pub fn get(&self, id: NodeId) -> Option<&T> {
        if self.is_valid_handle(id) {
            self.values[id.index() as usize].as_ref()
        } else {
            None
        }
    }

    /// Mutably borrow the value at `id`, or `None` if the handle is stale.
    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut T> {
        if self.is_valid_handle(id) {
            self.values[id.index() as usize].as_mut()
        } else {
            None
        }
    }

    /// Remove and return the value at `id`, bumping the slot's generation so that
    /// existing handles to it become stale.
    pub fn remove(&mut self, id: NodeId) -> Option<T> {
        let slot = id.index() as usize;
        if self.is_valid_handle(id) {
            self.generations[slot] += 1;
            self.free.push(slot as u32);
            self.values[slot].take()
        } else {
            None
        }
    }

    /// The number of live values currently stored.
    pub fn len(&self) -> usize {
        self.values.len() - self.free.len()
    }

    /// Whether the arena holds no live values.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Borrow every live value in the arena, in slot order.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.values.iter().filter_map(|value| value.as_ref())
    }
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_then_get_returns_the_value() {
        let mut arena: Arena<i32> = Arena::new();
        let id = arena.insert(42);
        assert_eq!(arena.get(id), Some(&42));
        assert_eq!(arena.len(), 1);
        assert!(!arena.is_empty());
    }

    #[test]
    fn get_returns_none_for_stale_handle_after_remove() {
        let mut arena: Arena<i32> = Arena::new();
        let id = arena.insert(7);
        assert_eq!(arena.remove(id), Some(7));
        assert_eq!(arena.get(id), None);
    }

    #[test]
    fn insert_reuses_freed_slot_with_bumped_generation() {
        let mut arena: Arena<i32> = Arena::new();
        let first = arena.insert(1);
        arena.remove(first);
        let second = arena.insert(2);

        // Same physical slot, but a higher generation distinguishes the occupants.
        assert_eq!(second.index(), first.index());
        assert!(second.generation() > first.generation());

        // The stale handle fails; the fresh one resolves.
        assert_eq!(arena.get(first), None);
        assert_eq!(arena.get(second), Some(&2));
    }

    #[test]
    fn remove_returns_value_once_then_none_afterwards() {
        let mut arena: Arena<i32> = Arena::new();
        let id = arena.insert(9);
        assert_eq!(arena.remove(id), Some(9));
        assert_eq!(arena.remove(id), None);
    }

    #[test]
    fn len_and_is_empty_track_live_values() {
        let mut arena: Arena<i32> = Arena::new();
        assert!(arena.is_empty());
        let first = arena.insert(1);
        let _second = arena.insert(2);
        assert_eq!(arena.len(), 2);
        arena.remove(first);
        assert_eq!(arena.len(), 1);
    }

    #[test]
    fn get_mut_allows_in_place_update() {
        let mut arena: Arena<i32> = Arena::new();
        let id = arena.insert(1);
        *arena.get_mut(id).unwrap() = 99;
        assert_eq!(arena.get(id), Some(&99));
        assert_eq!(arena.get_mut(id).map(|v| *v), Some(99));
    }

    #[test]
    fn default_arena_is_empty() {
        let arena: Arena<i32> = Arena::default();
        assert!(arena.is_empty());
        assert_eq!(arena.len(), 0);
    }
}
