pub type TigerHashMap<K,V> = ahash::HashMap<K,V>;
pub type TigerHashSet<T> = ahash::HashSet<T>;
pub(crate) type BiTigerHashMap<L,R> = bimap::BiHashMap<L,R,ahash::RandomState,ahash::RandomState>;
#[macro_export] macro_rules! set { ($x:expr) => { ahash::AHashSet::from($x).into() }; }
