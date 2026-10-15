use std::path::{Path,PathBuf};
use std::ffi::OsStr;
use std::fmt::{Display,Formatter};
use crate::pathtable::{PathTable,PathTableIndex};
/// Note that ordering of these enum values matters.
/// Files later in the order will override files of the same name before them,
/// and the warnings about duplicates take that into account.
// TODO: verify the relative order of `Clausewitz` and `Jomini`
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FileKind {
    /// `Internal` is for parsing tiger's own data. The user should not see warnings from this.
    Internal,
    /// `Clausewitz` and `Jomini` are directories bundled with the base game.
    Clausewitz,
    Jomini,
    /// The base game files.
    Vanilla,
    /// Downloadable content present on the user's system.
    Dlc(u8),
    /// Other mods loaded as directed by the config file. 0-based indexing.
    LoadedMod(u8),
    /// The mod under scrutiny. Usually, warnings are not emitted unless they touch `Mod` files.
    Mod,
}

impl FileKind {
    pub fn counts_as_vanilla(&self) -> bool {
        match self {
            FileKind::Clausewitz | FileKind::Jomini | FileKind::Vanilla | FileKind::Dlc(_) => true,
            FileKind::Internal | FileKind::LoadedMod(_) | FileKind::Mod => false,
        }
    }
}

/// The top level directories used by EU5.
/// The other games only have the `NoStage` stage, which doesn't correspond to a directory prefix.
///
/// Note that ordering of these enum values matters for the same reason as [`FileKind`].
#[derive(Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub enum FileStage {
    #[cfg(feature = "eu5")]
    LoadingScreen,
    #[cfg(feature = "eu5")]
    MainMenu,
    #[cfg(feature = "eu5")]
    InGame,
    NoStage,
}


#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileEntry {
    /// Pathname components below the mod directory or the vanilla game dir
    /// Must not be empty.
    path: PathBuf,
    /// The top-level directories use by EU5. This prefix is not included in `path`.
    stage: FileStage,
    /// Whether it's a vanilla or mod file
    kind: FileKind,
    /// Index into the `PathTable`. Used to initialize `Loc`, which doesn't carry a copy of the pathbuf.
    /// A `FileEntry` might not have this index, because `FileEntry` needs to be usable before the (ordered)
    /// path table is created.
    idx: Option<PathTableIndex>,
    /// The full filesystem path of this entry. Not used for ordering or equality.
    fullpath: PathBuf,
}

impl FileEntry {
    pub fn new(path: PathBuf, stage: FileStage, kind: FileKind, fullpath: PathBuf) -> Self {
        assert!(path.file_name().is_some());
        Self { path, stage, kind, idx: None, fullpath }
    }

    pub fn stage(&self) -> FileStage {
        self.stage
    }

    pub fn kind(&self) -> FileKind {
        self.kind
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn fullpath(&self) -> &Path {
        &self.fullpath
    }

    /// Convenience function
    /// Won't panic because `FileEntry` with empty filename is not allowed.
    #[allow(clippy::missing_panics_doc)]
    pub fn filename(&self) -> &OsStr {
        self.path.file_name().unwrap()
    }

    fn store_in_pathtable(&mut self) {
        assert!(self.idx.is_none());
        self.idx = Some(PathTable::store(self.path.clone(), self.fullpath.clone()));
    }

    pub fn path_idx(&self) -> Option<PathTableIndex> {
        self.idx
    }
}

impl Display for FileEntry {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), std::fmt::Error> {
        write!(fmt, "{}", self.path.display())
    }
}

