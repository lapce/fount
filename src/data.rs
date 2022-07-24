use crate::scan::scan_path;

use super::font::*;
use super::id::*;
use super::*;
use font_kit::handle::Handle;
use font_kit::source::SystemSource;
use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use swash::text::Cjk;
use swash::text::Script;
use swash::{Attributes, CacheKey, Stretch, Style, Weight};

#[derive(Clone)]
pub struct FamilyData {
    pub name: String,
    pub has_stretch: bool,
    pub fonts: Vec<(FontId, Stretch, Weight, Style)>,
}

#[derive(Clone)]
pub struct FontData {
    pub family: FamilyId,
    pub source: SourceId,
    pub index: u32,
    pub attributes: Attributes,
    pub cache_key: CacheKey,
}

#[derive(Clone)]
pub enum SourceDataKind {
    Path(Arc<PathBuf>),
    Data(super::font::FontData),
}

#[derive(Clone)]
pub enum SourceDataStatus {
    Vacant,
    Present(WeakFontData),
    Error,
}

pub struct SourceData {
    pub kind: SourceDataKind,
    pub status: RwLock<SourceDataStatus>,
}

impl SourceData {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, io::Error> {
        let path = path
            .as_ref()
            .to_str()
            .ok_or(io::Error::new(io::ErrorKind::NotFound, "not found"))?;
        Ok(SourceData {
            kind: SourceDataKind::Path(Arc::new(path.into())),
            status: RwLock::new(SourceDataStatus::Vacant),
        })
    }
}

impl Clone for SourceData {
    fn clone(&self) -> Self {
        Self {
            kind: self.kind.clone(),
            status: RwLock::new(self.status.read().unwrap().clone()),
        }
    }
}

#[derive(Clone)]
pub struct CollectionData {
    pub system_source: Arc<SystemSource>,
    pub is_user: bool,
    pub families: Vec<Arc<FamilyData>>,
    pub fonts: Vec<FontData>,
    pub sources: Vec<SourceData>,
    pub family_map: HashMap<Arc<str>, Option<FamilyId>>,
    pub default_families: Vec<FamilyId>,
    pub generic_families: [Vec<FamilyId>; GENERIC_FAMILY_COUNT],
    pub cjk_families: [Vec<FamilyId>; CJK_FAMILY_COUNT],
    pub script_fallbacks: HashMap<[u8; 4], Vec<FamilyId>>,
}

impl Default for CollectionData {
    fn default() -> Self {
        Self::new()
    }
}

impl CollectionData {
    pub fn new() -> Self {
        Self {
            system_source: Arc::new(SystemSource::new()),
            is_user: false,
            families: Vec::new(),
            fonts: Vec::new(),
            sources: Vec::new(),
            family_map: HashMap::new(),
            default_families: Vec::new(),
            generic_families: Default::default(),
            cjk_families: Default::default(),
            script_fallbacks: HashMap::new(),
        }
    }

    pub fn family_id(&mut self, name: &str) -> Option<FamilyId> {
        let mut lowercase_buf = LowercaseString::new();
        let lowercase_name = lowercase_buf.get(name)?;

        if !self.family_map.contains_key(lowercase_name) {
            if let Ok(handle) = self.system_source.select_family_by_name(name) {
                for font in handle.fonts() {
                    match font {
                        Handle::Path { path, font_index } => {
                            scan_path(path, self);
                        }
                        Handle::Memory {
                            bytes: _,
                            font_index: _,
                        } => {}
                    }
                }
            } else {
                self.family_map.insert(name.into(), None);
            }
        }

        if let Some(family_id) = self.family_map.get(lowercase_name) {
            *family_id
        } else {
            None
        }
    }

    pub fn family(&self, id: FamilyId) -> Option<FamilyEntry> {
        let family = self.families.get(id.to_usize())?;
        Some(FamilyEntry {
            id,
            has_stretch: family.has_stretch,
            kind: FontFamilyKind::Dynamic(family.clone()),
        })
    }

    pub fn family_by_name(&mut self, name: &str) -> Option<FamilyEntry> {
        let family_id = self.family_id(name)?;
        self.family(family_id)
    }

    pub fn generic_families(&self, family: GenericFamily) -> &[FamilyId] {
        self.generic_families
            .get(family as usize)
            .map(|families| families.as_ref())
            .unwrap_or(&[])
    }

    pub fn default_families(&self) -> &[FamilyId] {
        &self.default_families
    }

    pub fn fallback_families(&mut self, script: Script, locale: Option<Locale>) -> &[FamilyId] {
        if script == Script::Han {
            let cjk = locale.map(|l| l.cjk()).unwrap_or(Cjk::None);
            return &self.cjk_families[cjk as usize];
        }

        let tag = super::script_tags::script_tag(script);
        let entry = self.script_fallbacks.entry(tag).or_default();
        match self.script_fallbacks.get(&tag) {
            Some(families) => {
                // println!("families for {script:?} {families:?}");
                families
            }
            _ => &self.default_families,
        }
    }

    fn find_family(&mut self, families: &[&str]) -> Vec<FamilyId> {
        let mut family_ids = Vec::new();
        for family in families {
            if let Some(id) = self.family_id(*family) {
                family_ids.push(id)
            }
        }
        family_ids
    }

    pub fn setup_default(&mut self) {
        use super::system::*;
        let families = match OS {
            Os::Windows => self.find_family(&["segoe ui"]),
            Os::MacOs => self.find_family(&["helvetica"]),
            _ => self.find_family(&["Cantarell Regular", "liberation serif", "dejavu serif"]),
        };
        self.default_families = families;
    }

    pub fn setup_default_generic(&mut self) {
        use super::system::*;
        use GenericFamily::*;
        match OS {
            Os::Windows => {
                self.generic_families[SansSerif as usize] = self.find_family(&["arial"]);
                self.generic_families[Serif as usize] = self.find_family(&["times new roman"]);
                self.generic_families[Monospace as usize] = self.find_family(&["courier new"]);
                self.generic_families[Cursive as usize] = self.find_family(&["comic sans ms"]);
                self.generic_families[SystemUi as usize] = self.find_family(&["segoe ui"]);
                self.generic_families[Emoji as usize] = self.find_family(&["segoe ui emoji"]);
            }
            Os::MacOs => {
                self.generic_families[SansSerif as usize] = self.find_family(&["helvetica"]);
                self.generic_families[Serif as usize] = self.find_family(&["times"]);
                self.generic_families[Monospace as usize] = self.find_family(&["courier"]);
                self.generic_families[Cursive as usize] = self.find_family(&["apple chancery"]);
                self.generic_families[SystemUi as usize] = self.find_family(&["helvetica"]);
                self.generic_families[Emoji as usize] = self.find_family(&["apple color emoji"]);
            }
            _ => {
                self.generic_families[SansSerif as usize] = self.find_family(&["sans-serif"]);
                self.generic_families[Serif as usize] = self.find_family(&["serif"]);
                self.generic_families[Monospace as usize] = self.find_family(&["monospace"]);
                self.generic_families[Cursive as usize] = self.find_family(&["cursive"]);
                self.generic_families[SystemUi as usize] = self.find_family(&[
                    "system-ui",
                    "Cantarell Regular",
                    "liberation sans",
                    "dejavu sans",
                ]);
                self.generic_families[Emoji as usize] =
                    self.find_family(&["noto color emoji", "emoji one"]);
            }
        }
    }

    /// When we do find_family, these fonts will be added to fallbacks in scan_font
    pub fn setup_fallbacks(&mut self) {
        use super::system::*;
        match OS {
            Os::Windows => {
                let _ = self.find_family(&[
                    "microsoft yahei",
                    "simsun",
                    "simsun-extb",
                    "meiryo",
                    "yu gothic",
                    "microsoft jhenghei",
                    "pmingliu",
                    "pmingliu-extb",
                    "malgun gothic",
                    "gulim",
                ]);
            }
            Os::MacOs => {
                let _ = self.find_family(&[
                    "pingfang sc",
                    "geeza pro",
                    "hiragino maru gothic pron w4",
                    "hiragino kaku gothic pron w3",
                    "apple sd gothic neo",
                    "Menlo",
                    "STIXGeneral",
                ]);
            }
            _ => {
                let _ = self.find_family(&[
                    "Noto Sans CJK SC",
                    "Noto Sans CJK TC",
                    "Noto Sans CJK JP",
                    "Noto Sans CJK KR",
                ]);
            }
        }
    }

    pub fn font(&self, id: FontId) -> Option<FontEntry> {
        let font = self.fonts.get(id.to_usize())?;
        Some(FontEntry {
            id,
            family: font.family,
            source: font.source,
            index: font.index,
            attributes: font.attributes,
            cache_key: font.cache_key,
        })
    }

    pub fn source(&self, id: SourceId) -> Option<SourceEntry> {
        let source = self.sources.get(id.to_usize())?;
        Some(SourceEntry {
            id,
            kind: match &source.kind {
                SourceDataKind::Path(path) => SourceKind::Path(path.clone()),
                SourceDataKind::Data(data) => SourceKind::Data(data.clone()),
            },
        })
    }

    pub fn load(&self, id: SourceId) -> Option<super::font::FontData> {
        let index = id.to_usize();
        let source_data = self.sources.get(index)?;
        let path: &Path = match &source_data.kind {
            SourceDataKind::Data(data) => return Some(data.clone()),
            SourceDataKind::Path(path) => &*path,
        };
        let font = load_source(path, &source_data.status);
        font
    }

    pub fn clone_into(&self, other: &mut Self) {
        other.families.clear();
        other.fonts.clear();
        other.sources.clear();
        other.family_map.clear();
        other.families.extend(self.families.iter().cloned());
        other.fonts.extend(self.fonts.iter().cloned());
        other.sources.extend(self.sources.iter().cloned());
        for (name, families) in &self.family_map {
            other.family_map.insert(name.clone(), families.clone());
        }
    }
}

#[derive(Default)]
pub struct ScannedCollectionData {
    pub collection: CollectionData,
}

pub struct StaticCollection {
    pub data: &'static StaticCollectionData,
    pub cache_keys: Vec<CacheKey>,
    pub sources: Vec<RwLock<SourceDataStatus>>,
}

impl StaticCollection {
    pub fn new(data: &'static StaticCollectionData) -> Self {
        let cache_keys = (0..data.fonts.len())
            .map(|_| CacheKey::new())
            .collect::<Vec<_>>();
        let sources = (0..data.sources.len())
            .map(|_| RwLock::new(SourceDataStatus::Vacant))
            .collect::<Vec<_>>();
        Self {
            data,
            cache_keys,
            sources,
        }
    }

    pub fn family_id(&self, name: &str) -> Option<FamilyId> {
        let mut lowercase_buf = LowercaseString::new();
        let lowercase_name = lowercase_buf.get(name)?;
        match self
            .data
            .families
            .binary_search_by(|x| x.lowercase_name.cmp(&lowercase_name))
        {
            Ok(index) => Some(FamilyId::new(index as u32)),
            _ => None,
        }
    }

    pub fn fallback_families(&self, script: Script, locale: Option<Locale>) -> &[FamilyId] {
        if script == Script::Han {
            let cjk = locale.map(|l| l.cjk() as usize).unwrap_or(0);
            return self.data.cjk_families[cjk];
        }
        let tag = super::script_tags::script_tag(script);
        match self
            .data
            .script_fallbacks
            .binary_search_by(|x| x.script.cmp(&tag))
        {
            Ok(index) => self
                .data
                .script_fallbacks
                .get(index)
                .map(|x| x.families)
                .unwrap_or(&[]),
            _ => self.data.default_families,
        }
    }

    pub fn family_name(&self, id: FamilyId) -> Option<&'static str> {
        self.data
            .families
            .get(id.to_usize())
            .map(|family| family.name)
    }

    pub fn load(&self, id: SourceId) -> Option<super::font::FontData> {
        let index = id.to_usize();
        let paths = SourcePaths {
            inner: SourcePathsInner::Static(self.data.search_paths),
            pos: 0,
        };
        load_source(
            &self.data.sources.get(index)?.file_name,
            self.sources.get(index)?,
        )
    }
}

fn load_source(path: &Path, status: &RwLock<SourceDataStatus>) -> Option<super::font::FontData> {
    match &*status.read().unwrap() {
        SourceDataStatus::Present(data) => {
            if let Some(data) = data.upgrade() {
                return Some(data);
            }
        }
        SourceDataStatus::Error => return None,
        _ => {}
    }
    let mut status = status.write().unwrap();
    match &*status {
        SourceDataStatus::Present(data) => {
            if let Some(data) = data.upgrade() {
                return Some(data);
            }
        }
        SourceDataStatus::Error => return None,
        _ => {}
    }
    if let Ok(data) = super::font::FontData::from_file(path) {
        *status = SourceDataStatus::Present(data.downgrade());
        return Some(data);
    }
    *status = SourceDataStatus::Error;
    None
}

pub enum SystemCollectionData {
    Static(StaticCollection),
    Scanned(ScannedCollectionData),
}

impl SystemCollectionData {
    pub fn source_paths(&self) -> SourcePaths {
        match self {
            Self::Static(data) => SourcePaths {
                inner: SourcePathsInner::Static(data.data.search_paths),
                pos: 0,
            },
            Self::Scanned(data) => SourcePaths {
                inner: SourcePathsInner::Static(&[]),
                pos: 0,
            },
        }
    }

    pub fn family(&self, id: FamilyId) -> Option<FamilyEntry> {
        match self {
            Self::Static(data) => {
                let family = data.data.families.get(id.to_usize())?;
                Some(FamilyEntry {
                    id,
                    has_stretch: family.has_stretch,
                    kind: FontFamilyKind::Static(family.name, family.fonts),
                })
            }
            Self::Scanned(data) => data.collection.family(id),
        }
    }

    pub fn family_by_name(&mut self, name: &str) -> Option<FamilyEntry> {
        let family_id = self.family_id(name)?;
        self.family(family_id)
    }

    pub fn font(&self, id: FontId) -> Option<FontEntry> {
        match self {
            Self::Static(data) => {
                let index = id.to_usize();
                let font = data.data.fonts.get(index)?;
                let cache_key = *data.cache_keys.get(index)?;
                Some(FontEntry {
                    id,
                    family: font.family,
                    source: font.source,
                    index: font.index,
                    attributes: font.attributes,
                    cache_key,
                })
            }
            Self::Scanned(data) => data.collection.font(id),
        }
    }

    pub fn add_fonts(
        &mut self,
        data: super::font::FontData,
        source: SourceData,
        mut reg: Option<&mut Registration>,
    ) -> Option<u32> {
        match self {
            SystemCollectionData::Static(_) => None,
            SystemCollectionData::Scanned(collection) => {
                collection.collection.add_fonts(data, source, reg)
            }
        }
    }

    pub fn source(&self, id: SourceId) -> Option<SourceEntry> {
        match self {
            Self::Static(data) => {
                let source = data.data.sources.get(id.to_usize())?;
                Some(SourceEntry {
                    id,
                    kind: SourceKind::FileName(source.file_name.clone()),
                })
            }
            Self::Scanned(data) => data.collection.source(id),
        }
    }

    pub fn load(&self, id: SourceId) -> Option<super::font::FontData> {
        match self {
            Self::Static(data) => data.load(id),
            Self::Scanned(data) => data.collection.load(id),
        }
    }

    pub fn default_families(&self) -> &[FamilyId] {
        match self {
            Self::Static(data) => data.data.default_families,
            Self::Scanned(data) => data.collection.default_families(),
        }
    }

    pub fn generic_families(&self, family: GenericFamily) -> &[FamilyId] {
        match self {
            Self::Static(data) => data
                .data
                .generic_families
                .get(family as usize)
                .copied()
                .unwrap_or(&[]),
            Self::Scanned(data) => data.collection.generic_families(family),
        }
    }

    pub fn fallback_families(&mut self, script: Script, locale: Option<Locale>) -> &[FamilyId] {
        match self {
            Self::Static(data) => data.fallback_families(script, locale),
            Self::Scanned(data) => data.collection.fallback_families(script, locale),
        }
    }

    pub fn family_id(&mut self, name: &str) -> Option<FamilyId> {
        match self {
            Self::Static(data) => data.family_id(name),
            Self::Scanned(data) => data.collection.family_id(name),
        }
    }
}

pub struct StaticFamilyData {
    pub name: &'static str,
    pub lowercase_name: &'static str,
    pub has_stretch: bool,
    pub fonts: &'static [(FontId, Stretch, Weight, Style)],
}

pub struct StaticFontData {
    pub family: FamilyId,
    pub attributes: Attributes,
    pub source: SourceId,
    pub index: u32,
}

pub struct StaticSourceData {
    pub file_name: PathBuf,
}

pub struct StaticScriptFallbacks {
    pub script: [u8; 4],
    pub families: &'static [FamilyId],
}

const GENERIC_FAMILY_COUNT: usize = 6;
const CJK_FAMILY_COUNT: usize = 5;

pub struct StaticCollectionData {
    pub search_paths: &'static [&'static str],
    pub families: &'static [StaticFamilyData],
    pub fonts: &'static [StaticFontData],
    pub sources: &'static [StaticSourceData],
    pub default_families: &'static [FamilyId],
    pub script_fallbacks: &'static [StaticScriptFallbacks],
    pub generic_families: [&'static [FamilyId]; GENERIC_FAMILY_COUNT],
    pub cjk_families: [&'static [FamilyId]; CJK_FAMILY_COUNT],
}

impl StaticCollectionData {
    pub fn family_id(&self, name: &str) -> Option<FamilyId> {
        let mut lowercase_buf = LowercaseString::new();
        let lowercase_name = lowercase_buf.get(name)?;
        match self
            .families
            .binary_search_by(|x| x.lowercase_name.cmp(&lowercase_name))
        {
            Ok(index) => Some(FamilyId::new(index as u32)),
            _ => None,
        }
    }

    pub fn fallback_families(&self, script: Script, locale: Option<Locale>) -> &[FamilyId] {
        if script == Script::Han {
            let cjk = locale.map(|l| l.cjk() as usize).unwrap_or(0);
            return self.cjk_families[cjk];
        }
        let tag = super::script_tags::script_tag(script);
        match self
            .script_fallbacks
            .binary_search_by(|x| x.script.cmp(&tag))
        {
            Ok(index) => self
                .script_fallbacks
                .get(index)
                .map(|x| x.families)
                .unwrap_or(&[]),
            _ => self.default_families,
        }
    }

    pub fn family_name(&self, id: FamilyId) -> Option<&'static str> {
        self.families.get(id.to_usize()).map(|family| family.name)
    }
}

/// Iterator over file system paths that contain fonts.
///
/// This iterator is returned by the [`source_paths`](super::FontContext::source_paths) method
/// of [`FontContext`](super::FontContext).
#[derive(Copy, Clone)]
pub struct SourcePaths<'a> {
    inner: SourcePathsInner<'a>,
    pos: usize,
}

impl<'a> Iterator for SourcePaths<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner {
            SourcePathsInner::Static(paths) => {
                if self.pos > paths.len() {
                    None
                } else {
                    let pos = self.pos;
                    self.pos += 1;
                    paths.get(pos).copied()
                }
            }
            SourcePathsInner::Dynamic(paths) => {
                if self.pos > paths.len() {
                    None
                } else {
                    let pos = self.pos;
                    self.pos += 1;
                    paths.get(pos).map(|s| s.as_str())
                }
            }
        }
    }
}

#[derive(Copy, Clone)]
enum SourcePathsInner<'a> {
    Static(&'static [&'static str]),
    Dynamic(&'a Vec<String>),
}

pub struct LowercaseString {
    buf: [u8; 128],
    heap: String,
}

impl LowercaseString {
    pub fn new() -> Self {
        Self {
            buf: [0u8; 128],
            heap: Default::default(),
        }
    }

    pub fn get<'a>(&'a mut self, name: &str) -> Option<&'a str> {
        if name.len() <= self.buf.len() && name.is_ascii() {
            let mut end = 0;
            for c in name.as_bytes() {
                self.buf[end] = c.to_ascii_lowercase();
                end += 1;
            }
            std::str::from_utf8(&self.buf[..end]).ok()
        } else {
            self.heap = name.to_lowercase();
            Some(&self.heap)
        }
    }
}
