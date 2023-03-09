use std::path::Path;

use peniko::{Blob, WeakBlob};

/// Shared reference to owned font data.
#[derive(Clone, Debug)]
#[repr(transparent)]
pub struct FontData {
    inner: Blob<u8>,
}

impl FontData {
    /// Creates font data from the specified bytes.
    pub fn new(data: Vec<u8>) -> Self {
        Self { inner: data.into() }
    }

    pub fn data(&self) -> Blob<u8> {
        self.inner.clone()
    }

    /// Creates font data from the file at the specified path.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, std::io::Error> {
        let path = path.as_ref();
        let data = std::fs::read(path)?;
        Ok(Self { inner: data.into() })
    }

    /// Creates a new weak reference to the data.
    pub fn downgrade(&self) -> WeakFontData {
        WeakFontData {
            inner: self.inner.downgrade(),
        }
    }

    /// Returns the underlying bytes of the data.
    pub fn as_bytes(&self) -> &[u8] {
        self.inner.data()
    }
}

impl std::ops::Deref for FontData {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_bytes()
    }
}

impl AsRef<[u8]> for FontData {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

#[derive(Debug)]
enum FontDataInner {
    Memory(Vec<u8>),
}

impl FontDataInner {
    pub fn data(&self) -> &[u8] {
        match self {
            Self::Memory(data) => data,
        }
    }
}

/// Weak reference to owned font data.
#[derive(Clone)]
#[repr(transparent)]
pub struct WeakFontData {
    inner: WeakBlob<u8>,
}

impl WeakFontData {
    /// Upgrades the weak reference.
    pub fn upgrade(&self) -> Option<FontData> {
        Some(FontData {
            inner: self.inner.upgrade()?,
        })
    }
}
