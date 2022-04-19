use super::data::*;
use crate::scan::{scan_path, FontScanner};
use crate::system::{Os, OS};
use std::io;
use std::path::Path;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, RwLock};

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
                system,
                user: Arc::new(RwLock::new(user)),
                user_version: Arc::new(AtomicU64::new(0)),
            }),
        }
    }
}

impl Default for Library {
    fn default() -> Self {
        LibraryBuilder::default()
            .add_system_path()
            .add_user_path()
            .build()
    }
}

pub struct Inner {
    pub system: SystemCollectionData,
    pub user: Arc<RwLock<CollectionData>>,
    pub user_version: Arc<AtomicU64>,
}

/// Builder for configuring a font library.
#[derive(Default)]
pub struct LibraryBuilder {
    scanner: FontScanner,
    system: CollectionData,
    fallback: FallbackData,
}

impl LibraryBuilder {
    pub fn add_system_path(mut self) -> Self {
        match OS {
            Os::Windows => {
                if let Some(mut windir) = std::env::var_os("SYSTEMROOT") {
                    windir.push("\\Fonts\\");
                    scan_path(
                        windir,
                        &mut self.scanner,
                        &mut self.system,
                        &mut self.fallback,
                    );
                } else {
                    scan_path(
                        "C:\\Windows\\Fonts\\",
                        &mut self.scanner,
                        &mut self.system,
                        &mut self.fallback,
                    );
                }
            }
            Os::MacOs => {
                scan_path(
                    "/System/Library/Fonts/",
                    &mut self.scanner,
                    &mut self.system,
                    &mut self.fallback,
                );
                scan_path(
                    "/Library/Fonts/",
                    &mut self.scanner,
                    &mut self.system,
                    &mut self.fallback,
                );
            }
            Os::Ios => {
                scan_path(
                    "/System/Library/Fonts/",
                    &mut self.scanner,
                    &mut self.system,
                    &mut self.fallback,
                );
                scan_path(
                    "/Library/Fonts/",
                    &mut self.scanner,
                    &mut self.system,
                    &mut self.fallback,
                );
            }
            Os::Android => {
                scan_path(
                    "/system/fonts/",
                    &mut self.scanner,
                    &mut self.system,
                    &mut self.fallback,
                );
            }
            Os::Unix => {
                scan_path(
                    "/usr/share/fonts/",
                    &mut self.scanner,
                    &mut self.system,
                    &mut self.fallback,
                );
                scan_path(
                    "/usr/local/share/fonts/",
                    &mut self.scanner,
                    &mut self.system,
                    &mut self.fallback,
                );
            }
            Os::Other => {}
        }

        self
    }

    pub fn add_user_path(mut self) -> Self {
        match OS {
            Os::Windows => {}
            Os::MacOs => {
                if let Some(mut homedir) = std::env::var_os("HOME") {
                    homedir.push("/Library/Fonts/");
                    scan_path(
                        &homedir,
                        &mut self.scanner,
                        &mut self.system,
                        &mut self.fallback,
                    );
                }
            }
            Os::Ios => {}
            Os::Android => {}
            Os::Unix => {
                if let Some(mut homedir) = std::env::var_os("HOME") {
                    homedir.push("/.local/share/fonts/");
                    scan_path(
                        &homedir,
                        &mut self.scanner,
                        &mut self.system,
                        &mut self.fallback,
                    );
                }
            }
            Os::Other => {}
        }

        self
    }

    pub fn build(mut self) -> Library {
        self.system.setup_default_generic();
        self.system.setup_fallback(&mut self.fallback);
        let system = SystemCollectionData::Scanned(ScannedCollectionData {
            collection: self.system,
            fallback: self.fallback,
        });
        Library::new(system)
    }
}
