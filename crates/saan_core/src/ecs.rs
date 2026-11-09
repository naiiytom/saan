use std::any::{Any, TypeId};
use std::collections::{BTreeMap, HashMap};

/// Marker trait for any component ("Strand") that can be woven onto an entity in [`World`].
pub trait Strand: Any + Send + Sync + 'static {
    /// Human-readable strand type name used by the Loom scheduler and inspector.
    fn strand_name() -> &'static str
    where
        Self: Sized,
    {
        std::any::type_name::<Self>()
    }
}

impl<T: Any + Send + Sync + 'static> Strand for T {}

/// Identifier for a live entity ("Warp thread") in the [`World`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
pub struct EntityId {
    pub index: u32,
    pub generation: u32,
}

impl std::fmt::Display for EntityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "e{}v{}", self.index, self.generation)
    }
}

trait AnyColumn: Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn remove(&mut self, entity: EntityId);
    fn contains(&self, entity: EntityId) -> bool;
    fn len(&self) -> usize;
}

struct StrandColumn<T: Strand> {
    data: BTreeMap<EntityId, T>,
}

impl<T: Strand> Default for StrandColumn<T> {
    fn default() -> Self {
        Self {
            data: BTreeMap::new(),
        }
    }
}

impl<T: Strand> AnyColumn for StrandColumn<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn remove(&mut self, entity: EntityId) {
        self.data.remove(&entity);
    }

    fn contains(&self, entity: EntityId) -> bool {
        self.data.contains_key(&entity)
    }

    fn len(&self) -> usize {
        self.data.len()
    }
}

/// The ECS container holding all entities (Warp threads) and their attached [`Strand`] components.
#[derive(Default)]
pub struct World {
    generations: Vec<u32>,
    free_indices: Vec<u32>,
    alive: Vec<bool>,
    columns: HashMap<TypeId, Box<dyn AnyColumn>>,
    strand_names: BTreeMap<&'static str, TypeId>,
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }

    /// Spawns a new empty entity (Warp thread).
    pub fn spawn(&mut self) -> EntityId {
        if let Some(index) = self.free_indices.pop() {
            let idx = index as usize;
            self.alive[idx] = true;
            EntityId {
                index,
                generation: self.generations[idx],
            }
        } else {
            let index = self.generations.len() as u32;
            self.generations.push(0);
            self.alive.push(true);
            EntityId {
                index,
                generation: 0,
            }
        }
    }

    /// Despawns an entity and removes all of its attached strands.
    pub fn despawn(&mut self, entity: EntityId) -> bool {
        if !self.is_alive(entity) {
            return false;
        }
        let idx = entity.index as usize;
        self.alive[idx] = false;
        self.generations[idx] = self.generations[idx].wrapping_add(1);
        self.free_indices.push(entity.index);

        for col in self.columns.values_mut() {
            col.remove(entity);
        }
        true
    }

    /// Returns `true` if `entity` is currently alive in the world.
    pub fn is_alive(&self, entity: EntityId) -> bool {
        let idx = entity.index as usize;
        idx < self.alive.len() && self.alive[idx] && self.generations[idx] == entity.generation
    }

    /// Returns the total number of live entities.
    pub fn entity_count(&self) -> usize {
        self.alive.iter().filter(|&&alive| alive).count()
    }

    /// Returns an iterator over all live entities in deterministic order.
    pub fn entities(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.alive
            .iter()
            .enumerate()
            .filter_map(|(idx, &alive)| {
                if alive {
                    Some(EntityId {
                        index: idx as u32,
                        generation: self.generations[idx],
                    })
                } else {
                    None
                }
            })
    }

    /// Attaches or overwrites a [`Strand`] component on `entity`.
    pub fn weave_strand<T: Strand>(&mut self, entity: EntityId, strand: T) -> &mut Self {
        assert!(self.is_alive(entity), "Cannot weave strand onto dead entity {entity}");
        let type_id = TypeId::of::<T>();
        self.strand_names.insert(T::strand_name(), type_id);
        let col = self
            .columns
            .entry(type_id)
            .or_insert_with(|| Box::new(StrandColumn::<T>::default()));
        let typed = col
            .as_any_mut()
            .downcast_mut::<StrandColumn<T>>()
            .expect("TypeId mismatch in StrandColumn");
        typed.data.insert(entity, strand);
        self
    }

    /// Returns `true` if `entity` has a strand of type `T`.
    pub fn has_strand<T: Strand>(&self, entity: EntityId) -> bool {
        self.columns
            .get(&TypeId::of::<T>())
            .map(|col| col.contains(entity))
            .unwrap_or(false)
    }

    /// Retrieves an immutable reference to a [`Strand`] on `entity`.
    pub fn get_strand<T: Strand>(&self, entity: EntityId) -> Option<&T> {
        let col = self.columns.get(&TypeId::of::<T>())?;
        let typed = col.as_any().downcast_ref::<StrandColumn<T>>()?;
        typed.data.get(&entity)
    }

    /// Retrieves a mutable reference to a [`Strand`] on `entity`.
    pub fn get_strand_mut<T: Strand>(&mut self, entity: EntityId) -> Option<&mut T> {
        let col = self.columns.get_mut(&TypeId::of::<T>())?;
        let typed = col.as_any_mut().downcast_mut::<StrandColumn<T>>()?;
        typed.data.get_mut(&entity)
    }

    /// Removes a [`Strand`] from `entity` and returns it if present.
    pub fn unweave_strand<T: Strand>(&mut self, entity: EntityId) -> Option<T> {
        let col = self.columns.get_mut(&TypeId::of::<T>())?;
        let typed = col.as_any_mut().downcast_mut::<StrandColumn<T>>()?;
        typed.data.remove(&entity)
    }

    /// Iterates over all `(EntityId, &T)` pairs for a given strand type `T`.
    pub fn query<T: Strand>(&self) -> impl Iterator<Item = (EntityId, &T)> {
        self.columns
            .get(&TypeId::of::<T>())
            .and_then(|col| col.as_any().downcast_ref::<StrandColumn<T>>())
            .into_iter()
            .flat_map(|typed| typed.data.iter().map(|(&e, v)| (e, v)))
    }

    /// Iterates mutably over all `(EntityId, &mut T)` pairs for a given strand type `T`.
    pub fn query_mut<T: Strand>(&mut self) -> impl Iterator<Item = (EntityId, &mut T)> {
        self.columns
            .get_mut(&TypeId::of::<T>())
            .and_then(|col| col.as_any_mut().downcast_mut::<StrandColumn<T>>())
            .into_iter()
            .flat_map(|typed| typed.data.iter_mut().map(|(&e, v)| (e, v)))
    }

    /// Queries entities that have both strand `A` and strand `B`.
    pub fn query_pair<A: Strand, B: Strand>(&self) -> Vec<(EntityId, &A, &B)> {
        self.query::<A>()
            .filter_map(|(entity, a)| self.get_strand::<B>(entity).map(|b| (entity, a, b)))
            .collect()
    }

    /// Returns a summary map of registered strand type names to their active instance counts.
    pub fn strand_counts(&self) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();
        for (&name, &type_id) in &self.strand_names {
            if let Some(col) = self.columns.get(&type_id) {
                let short = name.rsplit("::").next().unwrap_or(name).to_string();
                counts.insert(short, col.len());
            }
        }
        counts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct Health(i32);

    #[derive(Debug, PartialEq)]
    struct Mana(i32);

    #[test]
    fn spawn_weave_and_query_strands() {
        let mut world = World::new();
        let e1 = world.spawn();
        let e2 = world.spawn();

        world.weave_strand(e1, Health(100)).weave_strand(e1, Mana(50));
        world.weave_strand(e2, Health(80));

        assert_eq!(world.entity_count(), 2);
        assert_eq!(world.get_strand::<Health>(e1), Some(&Health(100)));
        assert_eq!(world.get_strand::<Mana>(e2), None);

        let pairs = world.query_pair::<Health, Mana>();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].0, e1);
    }

    #[test]
    fn despawn_increments_generation_and_cleans_strands() {
        let mut world = World::new();
        let e1 = world.spawn();
        world.weave_strand(e1, Health(100));

        assert!(world.despawn(e1));
        assert!(!world.is_alive(e1));
        assert_eq!(world.get_strand::<Health>(e1), None);

        let e2 = world.spawn();
        assert_eq!(e2.index, e1.index);
        assert_eq!(e2.generation, e1.generation + 1);
    }
}
