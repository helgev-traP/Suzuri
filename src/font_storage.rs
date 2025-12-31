use std::{collections::HashMap, path::PathBuf, sync::Arc};

/// Manages font loading and retrieval using `fontdb` and `fontdue`.
///
/// This struct combines a database of available fonts (`fontdb`) with a cache of loaded
/// font instances (`fontdue`). It allows querying for fonts by family and properties,
/// and lazily loads the actual font data when requested.
pub struct FontStorage {
    /// This is the font set that has been loaded by fontdb.
    font_db: fontdb::Database,
    /// This is the font that has been loaded by fontdue.
    /// Not all fonts in fontdb are necessarily loaded here.
    loaded_font: HashMap<fontdb::ID, Arc<fontdue::Font>, fxhash::FxBuildHasher>,
}

impl Default for FontStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl FontStorage {
    /// Creates a new empty font storage.
    pub fn new() -> Self {
        Self {
            font_db: fontdb::Database::new(),
            loaded_font: HashMap::with_hasher(fxhash::FxBuildHasher::default()),
        }
    }
}

/// Loading fonts into fontdb and setting up fontdb.
impl FontStorage {
    /// Loads a font from binary data.
    pub fn load_font_binary(&mut self, data: impl Into<Vec<u8>>) {
        self.font_db.load_font_data(data.into());
    }

    /// Loads a font from a file path.
    pub fn load_font_file(&mut self, path: PathBuf) -> Result<(), std::io::Error> {
        self.font_db.load_font_file(path)
    }

    /// Loads all fonts from a directory.
    pub fn load_fonts_dir(&mut self, dir: PathBuf) {
        self.font_db.load_fonts_dir(dir)
    }

    /// Loads the system fonts.
    pub fn load_system_fonts(&mut self) {
        self.font_db.load_system_fonts();
    }

    /// Manually adds a face info.
    pub fn push_face_info(&mut self, info: fontdb::FaceInfo) {
        self.font_db.push_face_info(info);
    }

    /// Removes a face by ID.
    pub fn remove_face(&mut self, id: fontdb::ID) {
        self.font_db.remove_face(id);
        self.loaded_font.remove(&id);
    }

    /// Checks if the storage is empty.
    pub fn is_empty(&self) -> bool {
        self.font_db.is_empty()
    }

    /// Returns the number of loaded faces.
    pub fn len(&self) -> usize {
        self.font_db.len()
    }

    /// Sets the family name for the "serif" generic family.
    pub fn set_serif_family(&mut self, family: impl Into<String>) {
        self.font_db.set_serif_family(family);
    }

    /// Sets the family name for the "sans-serif" generic family.
    pub fn set_sans_serif_family(&mut self, family: impl Into<String>) {
        self.font_db.set_sans_serif_family(family);
    }

    /// Sets the family name for the "cursive" generic family.
    pub fn set_cursive_family(&mut self, family: impl Into<String>) {
        self.font_db.set_cursive_family(family);
    }

    /// Sets the family name for the "fantasy" generic family.
    pub fn set_fantasy_family(&mut self, family: impl Into<String>) {
        self.font_db.set_fantasy_family(family);
    }

    /// Sets the family name for the "monospace" generic family.
    pub fn set_monospace_family(&mut self, family: impl Into<String>) {
        self.font_db.set_monospace_family(family);
    }

    /// Returns the name of a family.
    pub fn family_name<'a>(&'a self, family: &'a fontdb::Family<'_>) -> &'a str {
        self.font_db.family_name(family)
    }

    pub fn with_face_data<F, R>(&self, id: fontdb::ID, f: F) -> Option<R>
    where
        F: FnOnce(&[u8], u32) -> R,
    {
        self.font_db.with_face_data(id, f)
    }
}

/// Get `Font`
impl FontStorage {
    /// Queries for a font matching the description.
    ///
    /// Returns the ID and the loaded font if found.
    pub fn query(&mut self, query: &fontdb::Query) -> Option<(fontdb::ID, Arc<fontdue::Font>)> {
        let id = self.font_db.query(query)?;
        self.font(id).map(|font| (id, font))
    }

    /// Retrieves a loaded font by ID, loading it if necessary.
    pub fn font(&mut self, id: fontdb::ID) -> Option<Arc<fontdue::Font>> {
        use std::collections::hash_map::Entry;

        match self.loaded_font.entry(id) {
            Entry::Occupied(entry) => Some(Arc::clone(entry.get())),
            Entry::Vacant(entry) => {
                let font_result = self.font_db.with_face_data(id, |data, index| {
                    fontdue::Font::from_bytes(
                        data,
                        fontdue::FontSettings {
                            collection_index: index,
                            scale: 40.0,
                            load_substitutions: true,
                        },
                    )
                })?;

                match font_result {
                    Ok(font) => {
                        let r: &mut Arc<fontdue::Font> = entry.insert(Arc::new(font));
                        Some(Arc::clone(r))
                    }
                    Err(e) => {
                        log::error!("Failed to load font (id: {:?}): {}", id, e);
                        None
                    }
                }
            }
        }
    }

    /// Returns an iterator over all available faces.
    pub fn faces(&self) -> impl Iterator<Item = &fontdb::FaceInfo> {
        self.font_db.faces()
    }

    /// Returns face info for an ID.
    pub fn face(&self, id: fontdb::ID) -> Option<&fontdb::FaceInfo> {
        self.font_db.face(id)
    }

    /// Returns the source of a face.
    pub fn face_source(&self, id: fontdb::ID) -> Option<(fontdb::Source, u32)> {
        self.font_db.face_source(id)
    }
}
