use std::collections::HashMap;
use std::num::NonZeroUsize;

use crate::font_storage::FontStorage;
use crate::glyph_id::GlyphId;

#[derive(Default, Clone, Copy)]
struct LruNode {
    newer: Option<usize>,
    older: Option<usize>,
}

struct VecAtlas<T: Default + Clone + Copy> {
    capacity: usize,
    block_size: usize,
    data: Vec<T>,

    lru_nodes: Vec<LruNode>,
    lru_head: Option<usize>,
    lru_tail: Option<usize>,
    lru_map: HashMap<GlyphId, usize, fxhash::FxBuildHasher>,
    lru_empties: Vec<usize>,
    lru_keys: Vec<Option<GlyphId>>,
}

impl<T: Default + Clone + Copy> VecAtlas<T> {
    fn new(capacity: NonZeroUsize, block_size: NonZeroUsize) -> Self {
        let capacity = capacity.get();
        let block_size = block_size.get();

        Self {
            capacity,
            block_size,
            data: vec![T::default(); capacity * block_size],
            lru_nodes: vec![LruNode::default(); capacity],
            lru_head: None,
            lru_tail: None,
            lru_map: HashMap::with_capacity_and_hasher(capacity, fxhash::FxBuildHasher::default()),
            lru_empties: (0..capacity).collect(),
            lru_keys: vec![None; capacity],
        }
    }

    fn clear(&mut self) {
        self.lru_map.clear();
        self.lru_empties.clear();
        self.lru_empties.extend(0..self.capacity);
        self.lru_keys.fill(None);
        self.lru_head = None;
        self.lru_tail = None;
    }
}

impl<T: Default + Clone + Copy> VecAtlas<T> {
    pub fn get_or_insert_with(&mut self, key: &GlyphId, f: impl FnOnce() -> Vec<T>) -> &[T] {
        if let Some(index) = self.lru_map.get(key).cloned() {
            self.move_to_front(key);

            let index_from = index * self.block_size;
            let index_to = index_from + self.block_size;
            &self.data[index_from..index_to]
        } else {
            let block_index = self.push_front(key);

            let index_from = block_index * self.block_size;

            let rasterized_data = f();
            let copy_len = rasterized_data.len().min(self.block_size);
            self.data[index_from..index_from + copy_len]
                .copy_from_slice(&rasterized_data[0..copy_len]);

            &self.data[index_from..index_from + copy_len]
        }
    }
}

/// internal helpers
impl<T: Default + Clone + Copy> VecAtlas<T> {
    fn attach_to_head(&mut self, node_idx: usize, key: GlyphId) {
        // set node
        self.lru_nodes[node_idx].newer = None;
        self.lru_nodes[node_idx].older = self.lru_head;
        self.lru_map.insert(key, node_idx);
        self.lru_keys[node_idx] = Some(key);

        // update old head
        if let Some(old_head_idx) = self.lru_head {
            self.lru_nodes[old_head_idx].newer = Some(node_idx);
        }

        // update new head and tail
        self.lru_head = Some(node_idx);
        if self.lru_tail.is_none() {
            self.lru_tail = Some(node_idx);
        }
    }

    fn push_front(&mut self, key: &GlyphId) -> usize {
        if self.lru_map.contains_key(key) {
            panic!("key already exists");
        }

        let target_idx = if self.lru_empties.is_empty() {
            // all slots are used, evict tail
            let tail_idx = self
                .lru_tail
                .expect("tail must be set when all slots are used");

            // --- remove tail ---
            if let Some(second_tail) = self.lru_nodes[tail_idx].newer {
                self.lru_nodes[second_tail].older = None;
                self.lru_tail = Some(second_tail);
            } else {
                // tail == head (capacity 1)
                self.lru_head = None;
                self.lru_tail = None;
            }

            // remove from map
            if let Some(old_key) = self.lru_keys[tail_idx] {
                self.lru_map.remove(&old_key);
            }

            tail_idx
        } else {
            // use empty slot
            self.lru_empties.pop().expect("checked before")
        };

        // --- add head ---
        self.attach_to_head(target_idx, *key);

        target_idx
    }

    fn move_to_front(&mut self, key: &GlyphId) {
        // validate
        let Some(&current_index) = self.lru_map.get(key) else {
            return;
        };

        let older_idx = self.lru_nodes[current_index].older;
        let newer_idx = self.lru_nodes[current_index].newer;

        match (newer_idx, older_idx) {
            (Some(newer_idx), Some(older_idx)) => {
                // node is at middle

                // concatenate older and newer nodes
                self.lru_nodes[older_idx].newer = Some(newer_idx);
                self.lru_nodes[newer_idx].older = Some(older_idx);

                // update head
                let old_head_idx = self
                    .lru_head
                    .expect("there are more than 3 nodes. head must be set");
                self.lru_nodes[old_head_idx].newer = Some(current_index);
                self.lru_head = Some(current_index);

                // update current node
                self.lru_nodes[current_index].older = Some(old_head_idx);
                self.lru_nodes[current_index].newer = None;
            }
            (Some(newer_idx), None) => {
                // node is at tail

                // update tail
                self.lru_nodes[newer_idx].older = None;
                self.lru_tail = Some(newer_idx);

                // update head
                let old_head_idx = self
                    .lru_head
                    .expect("there are more than 2 nodes. head must be set");
                self.lru_nodes[old_head_idx].newer = Some(current_index);
                self.lru_head = Some(current_index);

                // update current node
                self.lru_nodes[current_index].older = Some(old_head_idx);
                self.lru_nodes[current_index].newer = None;
            }
            (None, _) => {
                // current node already at head
                // nothing to do
            }
        }
    }
}

use std::borrow::Cow;

pub struct CpuCacheItem<'a> {
    pub width: usize,
    pub height: usize,
    pub data: Cow<'a, [u8]>,
}

#[derive(Clone, Copy)]
pub struct CpuCacheConfig {
    pub block_size: NonZeroUsize,
    pub capacity: NonZeroUsize,
}

pub struct CpuCache {
    /// must be sorted by block size
    caches: Vec<VecAtlas<u8>>,
}

impl CpuCache {
    pub fn new(configs: &[CpuCacheConfig]) -> Self {
        let sorted_by_blocsize = {
            let mut v = configs.to_vec();
            v.sort_by_key(|config| config.block_size);
            v
        };

        let caches = sorted_by_blocsize
            .into_iter()
            .map(|config| VecAtlas::new(config.capacity, config.block_size))
            .collect();

        Self { caches }
    }

    pub fn clear(&mut self) {
        for cache in &mut self.caches {
            cache.clear();
        }
    }

    pub fn get(
        &'_ mut self,
        glyph_id: &GlyphId,
        font_storage: &mut FontStorage,
    ) -> Option<CpuCacheItem<'_>> {
        let glyph_index = glyph_id.glyph_index();
        let font_size = glyph_id.font_size();
        let font_id = glyph_id.font_id();

        let font = font_storage.font(font_id)?;
        let glyph_metrics = font.metrics_indexed(glyph_index, font_size);
        let glyph_bitmap_size = glyph_metrics.width * glyph_metrics.height;

        let cache = self
            .caches
            .iter_mut()
            .find(|cache| cache.block_size >= glyph_bitmap_size)?;

        let data = cache.get_or_insert_with(glyph_id, || {
            let bitmap = font.rasterize_indexed(glyph_index, font_size);
            bitmap.1
        });

        Some(CpuCacheItem {
            width: glyph_metrics.width,
            height: glyph_metrics.height,
            data: Cow::Borrowed(data),
        })
    }
}

#[allow(clippy::unwrap_used)]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::glyph_id::GlyphId;
    use std::num::NonZeroUsize;

    // Helper to create a dummy GlyphId
    fn make_key(id: u16) -> GlyphId {
        // fontdb::ID is 64-bit on this platform based on the error.
        // It might be NonZero, so use 1.
        let font_id: fontdb::ID = unsafe { std::mem::transmute(1u64) };
        GlyphId::new(font_id, id, 12.0)
    }

    #[test]
    fn test_vec_atlas_basic() {
        let capacity = NonZeroUsize::new(2).unwrap();
        let block_size = NonZeroUsize::new(4).unwrap();
        let mut atlas: VecAtlas<u8> = VecAtlas::new(capacity, block_size);

        let key1 = make_key(1);

        // Insert
        // lru_emptys = [0, 1]. pop() -> 1.
        let data = atlas.get_or_insert_with(&key1, || vec![1, 2, 3, 4]);
        assert_eq!(data, &[1, 2, 3, 4]);
        assert_eq!(atlas.lru_map.len(), 1);
        assert_eq!(atlas.lru_head, Some(1)); // First slot is 1
        assert_eq!(atlas.lru_tail, Some(1));

        // Get cached
        let data = atlas.get_or_insert_with(&key1, || vec![9, 9, 9, 9]);
        assert_eq!(data, &[1, 2, 3, 4]);
        assert_eq!(atlas.lru_map.len(), 1);
    }

    #[test]
    fn test_vec_atlas_eviction() {
        let capacity = NonZeroUsize::new(2).unwrap();
        let block_size = NonZeroUsize::new(1).unwrap();
        let mut atlas: VecAtlas<u8> = VecAtlas::new(capacity, block_size);

        let key1 = make_key(1);
        let key2 = make_key(2);
        let key3 = make_key(3);

        // Insert 1 -> index 1
        atlas.get_or_insert_with(&key1, || vec![1]);
        assert_eq!(atlas.lru_head, Some(1));
        assert_eq!(atlas.lru_tail, Some(1));

        // Insert 2 -> index 0
        atlas.get_or_insert_with(&key2, || vec![2]);
        assert_eq!(atlas.lru_map.len(), 2);
        assert_eq!(atlas.lru_head, Some(0)); // Newest is head (0)
        assert_eq!(atlas.lru_tail, Some(1)); // Oldest is tail (1)

        // Check links
        // Head (0) -> older should be 1
        assert_eq!(atlas.lru_nodes[0].older, Some(1));
        assert_eq!(atlas.lru_nodes[0].newer, None);
        // Tail (1) -> newer should be 0
        assert_eq!(atlas.lru_nodes[1].newer, Some(0));
        assert_eq!(atlas.lru_nodes[1].older, None);

        // Insert 3 (should evict key1 which is at tail 1)
        atlas.get_or_insert_with(&key3, || vec![3]);
        assert_eq!(atlas.lru_map.len(), 2);
        assert!(atlas.lru_map.contains_key(&key2));
        assert!(atlas.lru_map.contains_key(&key3));
        assert!(!atlas.lru_map.contains_key(&key1));

        // key3 should be head, key2 should be tail
        // Logic: swap_idx = tail (1).
        // key3 uses slot 1.
        // key3 is new head. key2 is new tail.

        let head_idx = atlas.lru_head.unwrap();
        let tail_idx = atlas.lru_tail.unwrap();

        assert_eq!(head_idx, 1); // Slot 1 reused for key3
        assert_eq!(tail_idx, 0); // Slot 0 (key2) is now tail

        assert_eq!(atlas.lru_keys[1], Some(key3));
        assert_eq!(atlas.lru_keys[0], Some(key2));
    }

    #[test]
    fn test_vec_atlas_update_lru() {
        let capacity = NonZeroUsize::new(3).unwrap();
        let block_size = NonZeroUsize::new(1).unwrap();
        let mut atlas: VecAtlas<u8> = VecAtlas::new(capacity, block_size);

        let key1 = make_key(1);
        let key2 = make_key(2);
        let key3 = make_key(3);

        // emptys: [0, 1, 2]
        atlas.get_or_insert_with(&key1, || vec![1]); // Head: 2. Tail: 2.
        atlas.get_or_insert_with(&key2, || vec![2]); // Head: 1. Tail: 2.
        atlas.get_or_insert_with(&key3, || vec![3]); // Head: 0. Tail: 2. Mid: 1.

        // Access key1 (tail, 2) -> should move to head
        atlas.get_or_insert_with(&key1, || vec![99]);

        // Expected order: 1 (Head, 2), 3 (0), 2 (Tail, 1)

        let head = atlas.lru_head.unwrap();
        let tail = atlas.lru_tail.unwrap();

        assert_eq!(head, 2);
        assert_eq!(tail, 1);

        assert_eq!(atlas.lru_keys[head], Some(key1));
        assert_eq!(atlas.lru_keys[tail], Some(key2));

        // Check middle (0)
        let mid = atlas.lru_nodes[head].older.unwrap();
        assert_eq!(mid, 0);
        assert_eq!(atlas.lru_keys[mid], Some(key3));
    }

    #[test]
    fn test_vec_atlas_capacity_1() {
        let capacity = NonZeroUsize::new(1).unwrap();
        let block_size = NonZeroUsize::new(1).unwrap();
        let mut atlas: VecAtlas<u8> = VecAtlas::new(capacity, block_size);

        let key1 = make_key(1);
        let key2 = make_key(2);

        atlas.get_or_insert_with(&key1, || vec![1]);
        assert_eq!(atlas.lru_head, Some(0));
        assert_eq!(atlas.lru_tail, Some(0));

        atlas.get_or_insert_with(&key2, || vec![2]);
        assert_eq!(atlas.lru_head, Some(0));
        assert_eq!(atlas.lru_tail, Some(0));
        assert!(atlas.lru_map.contains_key(&key2));
        assert!(!atlas.lru_map.contains_key(&key1));
    }

    #[test]
    fn test_glyph_cache_selection() {
        let config = vec![
            CpuCacheConfig {
                block_size: NonZeroUsize::new(10).unwrap(),
                capacity: NonZeroUsize::new(100).unwrap(),
            },
            CpuCacheConfig {
                block_size: NonZeroUsize::new(20).unwrap(),
                capacity: NonZeroUsize::new(50).unwrap(),
            },
        ];

        let cache = CpuCache::new(&config);
        assert_eq!(cache.caches.len(), 2);
        assert_eq!(cache.caches[0].block_size, 10);
        assert_eq!(cache.caches[1].block_size, 20);
    }
}
