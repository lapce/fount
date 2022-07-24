use super::data::*;
use crate::scan::FontScanner;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

/// Indexed collection of fonts and associated metadata supporting queries and
/// fallback.
///
/// This struct is opaque and provides shared storage for a font collection.
/// Accessing the collection is done by creating a [`FontContext`](super::context::FontContext)
/// wrapping this struct.
#[derive(Clone)]
pub struct Library {
    pub(crate) inner: Arc<Inner>,
}

impl Library {
    fn new(system: SystemCollectionData) -> Self {
        let mut user = CollectionData::default();
        user.is_user = true;
        Self {
            inner: Arc::new(Inner {
                system: Rc::new(RefCell::new(system)),
                user: Rc::new(RefCell::new(user)),
                user_version: Arc::new(AtomicU64::new(0)),
            }),
        }
    }
}

impl Default for Library {
    fn default() -> Self {
        LibraryBuilder::default().build()
    }
}

pub struct Inner {
    pub system: Rc<RefCell<SystemCollectionData>>,
    pub user: Rc<RefCell<CollectionData>>,
    pub user_version: Arc<AtomicU64>,
}

/// Builder for configuring a font library.
#[derive(Default)]
pub struct LibraryBuilder {
    scanner: FontScanner,
    system: CollectionData,
}

impl LibraryBuilder {
    pub fn build(mut self) -> Library {
        self.system.setup_default();
        self.system.setup_default_generic();
        self.system.setup_fallbacks();
        let system = SystemCollectionData::Scanned(ScannedCollectionData {
            collection: self.system,
        });
        Library::new(system)
    }
}
