use std::{collections::HashMap, path::PathBuf};

pub struct FontStorage {
    /// This is the font set that has been loaded by fontdb.
    font_db: fontdb::Database,
    /// This is the font that has been loaded by fontdue.
    /// Not all fonts in fontdb are necessarily loaded here.
    loaded_font: HashMap<fontdb::ID, fontdue::Font, fxhash::FxBuildHasher>,
}

impl Default for FontStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl FontStorage {
    pub fn new() -> Self {
        Self {
            font_db: fontdb::Database::new(),
            loaded_font: HashMap::with_hasher(fxhash::FxBuildHasher::default()),
        }
    }
}

/// Loading fonts into fontdb and setting up fontdb.
impl FontStorage {
    pub fn load_font_binary(&mut self, data: impl Into<Vec<u8>>) {
        self.font_db.load_font_data(data.into());
    }

    pub fn load_font_file(&mut self, path: PathBuf) -> Result<(), std::io::Error> {
        self.font_db.load_font_file(path)
    }

    pub fn load_fonts_dir(&mut self, dir: PathBuf) {
        self.font_db.load_fonts_dir(dir)
    }

    pub fn load_system_fonts(&mut self) {
        self.font_db.load_system_fonts();
    }

    pub fn push_face_info(&mut self, info: fontdb::FaceInfo) {
        self.font_db.push_face_info(info);
    }

    pub fn remove_face(&mut self, id: fontdb::ID) {
        self.font_db.remove_face(id);
        self.loaded_font.remove(&id);
    }

    pub fn is_empty(&self) -> bool {
        self.font_db.is_empty()
    }

    pub fn len(&self) -> usize {
        self.font_db.len()
    }

    pub fn set_serif_family(&mut self, family: impl Into<String>) {
        self.font_db.set_serif_family(family);
    }

    pub fn set_sans_serif_family(&mut self, family: impl Into<String>) {
        self.font_db.set_sans_serif_family(family);
    }

    pub fn set_cursive_family(&mut self, family: impl Into<String>) {
        self.font_db.set_cursive_family(family);
    }

    pub fn set_fantasy_family(&mut self, family: impl Into<String>) {
        self.font_db.set_fantasy_family(family);
    }

    pub fn set_monospace_family(&mut self, family: impl Into<String>) {
        self.font_db.set_monospace_family(family);
    }

    pub fn family_name<'a>(&'a self, family: &'a fontdb::Family<'_>) -> &'a str {
        self.font_db.family_name(family)
    }
}

/// Get `Font`
impl FontStorage {
    pub fn query_font(&mut self, query: &fontdb::Query) -> Option<&fontdue::Font> {
        let id = self.font_db.query(query)?;
        self.font(id)
    }

    pub fn query_id(&self, query: &fontdb::Query) -> Option<fontdb::ID> {
        self.font_db.query(query)
    }

    pub fn font(&mut self, id: fontdb::ID) -> Option<&fontdue::Font> {
        use std::collections::hash_map::Entry;

        match self.loaded_font.entry(id) {
            Entry::Occupied(entry) => Some(entry.into_mut()),
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
                let font = font_result.ok()?;

                // insert and return a reference that borrows from the map
                let r: &mut fontdue::Font = entry.insert(font);
                Some(&*r)
            }
        }
    }

    pub fn faces(&self) -> impl Iterator<Item = &fontdb::FaceInfo> {
        self.font_db.faces()
    }

    pub fn face(&self, id: fontdb::ID) -> Option<&fontdb::FaceInfo> {
        self.font_db.face(id)
    }

    pub fn face_source(&self, id: fontdb::ID) -> Option<(fontdb::Source, u32)> {
        self.font_db.face_source(id)
    }
}
