use euclid::{Box2D, Point2D, UnknownUnit};
use std::collections::HashMap;
use std::num::NonZeroUsize;

use crate::font_storage::FontStorage;
use crate::glyph_id::GlyphId;

/// protect `push_front`, `move_to_front` and `attach_to_head` from incorrect usage.
mod cache_state {
    use super::*;

    #[derive(Default, Clone, Copy)]
    struct LruNode {
        glyph_id: Option<GlyphId>,
        newer: Option<usize>,
        older: Option<usize>,
        last_used_batch_id: usize,
    }

    pub struct CacheState {
        capacity: usize,

        lru_nodes: Vec<LruNode>,
        lru_head: Option<usize>,
        lru_tail: Option<usize>,
        lru_map: HashMap<GlyphId, usize, fxhash::FxBuildHasher>,
        lru_empties: Vec<usize>,

        current_batch_id: usize,
    }

    pub enum GetOrPushResult {
        Hit,
        NeedToUpload,
    }

    impl CacheState {
        pub fn new(capacity: NonZeroUsize) -> Self {
            let capacity = capacity.get();
            Self {
                capacity,
                lru_nodes: vec![LruNode::default(); capacity],
                lru_head: None,
                lru_tail: None,
                lru_map: HashMap::with_capacity_and_hasher(
                    capacity,
                    fxhash::FxBuildHasher::default(),
                ),
                lru_empties: (0..capacity).collect(),
                current_batch_id: 0,
            }
        }

        pub fn clear(&mut self) {
            self.lru_map.clear();
            self.lru_empties.clear();
            self.lru_empties.extend(0..self.capacity);
            self.lru_head = None;
            self.lru_tail = None;
            self.current_batch_id = 0;
        }
    }
    impl CacheState {
        pub fn new_batch(&mut self) {
            self.current_batch_id = self.current_batch_id.wrapping_add(1);
        }

        pub fn get_or_push_and_protect(
            &mut self,
            glyph_id: &GlyphId,
        ) -> Option<(usize, GetOrPushResult)> {
            if let Some(idx) = self.get_and_protect_entry(glyph_id) {
                Some((idx, GetOrPushResult::Hit))
            } else {
                let idx = self.push_and_evicting_unprotected(glyph_id)?;
                Some((idx, GetOrPushResult::NeedToUpload))
            }
        }

        pub fn get_and_protect_entry(&mut self, glyph_id: &GlyphId) -> Option<usize> {
            if let Some(&idx) = self.lru_map.get(glyph_id) {
                // update last used frame
                let node = &mut self.lru_nodes[idx];
                node.last_used_batch_id = self.current_batch_id;

                // move to front
                self.move_to_front(glyph_id);

                Some(idx)
            } else {
                None
            }
        }

        pub fn push_and_evicting_unprotected(&mut self, glyph_id: &GlyphId) -> Option<usize> {
            if let Some(tail_idx) = self.lru_tail {
                let tail_node = &mut self.lru_nodes[tail_idx];
                if tail_node.last_used_batch_id == self.current_batch_id {
                    // tail is protected
                    return None;
                }
                // if tail is not protected, able to use push_front.
            }
            // there is no tail. means there is no entry in cache
            // able to use push_front.

            let allocated_idx = self.push_front(*glyph_id);
            let allocated_node = &mut self.lru_nodes[allocated_idx];
            allocated_node.last_used_batch_id = self.current_batch_id;

            Some(allocated_idx)
        }
    }

    /// internal helpers to opperate lru's linked list
    impl CacheState {
        fn push_front(&mut self, glyph_id: GlyphId) -> usize {
            if self.lru_map.contains_key(&glyph_id) {
                panic!("glyph_id already exists");
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
                if let Some(old_key) = self.lru_nodes[tail_idx].glyph_id {
                    self.lru_map.remove(&old_key);
                }

                tail_idx
            } else {
                // use empty slot
                self.lru_empties.pop().expect("checked before")
            };

            // --- add head ---
            self.attach_to_head(target_idx, glyph_id);

            target_idx
        }

        fn move_to_front(&mut self, glyph_id: &GlyphId) {
            // validate
            let Some(&current_index) = self.lru_map.get(glyph_id) else {
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

        fn attach_to_head(&mut self, node_idx: usize, glyph_id: GlyphId) {
            // set node
            self.lru_nodes[node_idx].newer = None;
            self.lru_nodes[node_idx].older = self.lru_head;
            self.lru_nodes[node_idx].glyph_id = Some(glyph_id);
            self.lru_map.insert(glyph_id, node_idx);

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
    }
}

pub struct GlyphAtlasConfig {
    pub tile_size: NonZeroUsize,
    pub tiles_per_axis: NonZeroUsize,
    pub texture_size: NonZeroUsize,
}

pub struct CacheAtlas {
    // square
    tile_size: usize,
    tiles_per_axis: usize,
    texture_size: usize,

    cache_state: cache_state::CacheState,
}

impl CacheAtlas {
    /// # Panics
    /// When:
    /// - tile_size * tiles_per_axis > texture_size
    /// - texture_size^2 > usize::MAX
    #[allow(clippy::unwrap_used)]
    fn new(config: GlyphAtlasConfig) -> Self {
        if config.tile_size.get() * config.tiles_per_axis.get() > config.texture_size.get() {
            panic!("tile_size * tiles_per_axis > texture_size");
        }

        let Some(cache_capacity) = config.tiles_per_axis.get().checked_pow(2) else {
            panic!("texture_size^2 > usize::MAX");
        };
        let cache_capacity = NonZeroUsize::new(cache_capacity).unwrap();

        Self {
            tile_size: config.tile_size.get(),
            tiles_per_axis: config.tiles_per_axis.get(),
            texture_size: config.texture_size.get(),
            cache_state: cache_state::CacheState::new(cache_capacity),
        }
    }

    fn clear(&mut self) {
        self.cache_state.clear();
    }
}

impl CacheAtlas {
    fn new_batch(&mut self) {
        self.cache_state.new_batch();
    }

    fn get_and_protect_entry(&mut self, glyph_id: &GlyphId) -> Option<[usize; 2]> {
        let index = self.cache_state.get_and_protect_entry(glyph_id)?;
        let x = (index % self.tiles_per_axis) * self.tile_size;
        let y = (index / self.tiles_per_axis) * self.tile_size;
        Some([x, y])
    }

    fn get_and_push_with_evicting_unprotected(&mut self, glyph_id: &GlyphId) -> Option<[usize; 2]> {
        let index = self.cache_state.push_and_evicting_unprotected(glyph_id)?;
        let x = (index % self.tiles_per_axis) * self.tile_size;
        let y = (index / self.tiles_per_axis) * self.tile_size;
        Some([x, y])
    }
}

pub struct GlyphCacheItem {
    pub texture_index: usize,
    pub texture_size: usize,
    pub glyph_box: Box2D<usize, UnknownUnit>,
}

impl GlyphCacheItem {
    pub const fn glyph_uv(&self) -> Box2D<f32, UnknownUnit> {
        let x_min = self.glyph_box.min.x;
        let x_max = self.glyph_box.max.x;
        let y_min = self.glyph_box.min.y;
        let y_max = self.glyph_box.max.y;
        Box2D::new(
            Point2D::new(
                x_min as f32 / self.texture_size as f32,
                y_min as f32 / self.texture_size as f32,
            ),
            Point2D::new(
                x_max as f32 / self.texture_size as f32,
                y_max as f32 / self.texture_size as f32,
            ),
        )
    }
}

pub enum GetOrPushResult {
    Hit,
    NeedToUpload,
}

pub struct GlyphCache {
    /// must be sorted by tile size
    caches: Vec<CacheAtlas>,
}

impl GlyphCache {
    pub fn new(configs: Vec<GlyphAtlasConfig>) -> Self {
        // sort by tile size
        let mut configs = configs;
        configs.sort_by_key(|config| config.tile_size.get());

        Self {
            caches: configs.into_iter().map(CacheAtlas::new).collect(),
        }
    }

    pub fn clear(&mut self) {
        for cache in &mut self.caches {
            cache.clear();
        }
    }
}

impl GlyphCache {
    pub fn new_batch(&mut self) {
        for cache in &mut self.caches {
            cache.new_batch();
        }
    }

    pub fn get_or_push_and_protect(
        &mut self,
        glyph_id: &GlyphId,
        font_storage: &mut FontStorage,
    ) -> Option<(GlyphCacheItem, GetOrPushResult)> {
        if let Some(item) = self.get_and_protect_entry(glyph_id, font_storage) {
            return Some((item, GetOrPushResult::Hit));
        }

        if let Some(item) = self.push_and_evicting_unprotected(glyph_id, font_storage) {
            return Some((item, GetOrPushResult::NeedToUpload));
        }

        None
    }

    pub fn get_and_protect_entry(
        &mut self,
        glyph_id: &GlyphId,
        font_storage: &mut FontStorage,
    ) -> Option<GlyphCacheItem> {
        let glyph_index = glyph_id.glyph_index();
        let font_size = glyph_id.font_size();
        let font_id = glyph_id.font_id();

        let font = font_storage.font(font_id)?;
        let glyph_metrics = font.metrics_indexed(glyph_index, font_size);
        let glyph_bitmap_size = glyph_metrics.width.max(glyph_metrics.height);

        let cache_index = self
            .caches
            .iter()
            .position(|cache| glyph_bitmap_size <= cache.tile_size)?;

        let cache = &mut self.caches[cache_index];
        let texture_index = cache_index;
        let texture_size = cache.texture_size;
        let [x_min, y_min] = cache.get_and_protect_entry(glyph_id)?;
        let x_max = x_min + glyph_metrics.width;
        let y_max = y_min + glyph_metrics.height;

        let glyph_box = Box2D::new(Point2D::new(x_min, y_min), Point2D::new(x_max, y_max));

        Some(GlyphCacheItem {
            texture_index,
            texture_size,
            glyph_box,
        })
    }

    pub fn push_and_evicting_unprotected(
        &mut self,
        glyph_id: &GlyphId,
        font_storage: &mut FontStorage,
    ) -> Option<GlyphCacheItem> {
        let glyph_index = glyph_id.glyph_index();
        let font_size = glyph_id.font_size();
        let font_id = glyph_id.font_id();

        let font = font_storage.font(font_id)?;
        let glyph_metrics = font.metrics_indexed(glyph_index, font_size);
        let glyph_bitmap_size = glyph_metrics.width.max(glyph_metrics.height);

        let cache_index = self
            .caches
            .iter()
            .position(|cache| glyph_bitmap_size <= cache.tile_size)?;

        let cache = &mut self.caches[cache_index];
        let texture_index = cache_index;
        let texture_size = cache.texture_size;
        let [x_min, y_min] = cache.get_and_push_with_evicting_unprotected(glyph_id)?;
        let x_max = x_min + glyph_metrics.width;
        let y_max = y_min + glyph_metrics.height;

        let glyph_box = Box2D::new(Point2D::new(x_min, y_min), Point2D::new(x_max, y_max));

        Some(GlyphCacheItem {
            texture_index,
            texture_size,
            glyph_box,
        })
    }
}
