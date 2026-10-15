//! Stellar-only lexical profile. No other game backend is available.
pub struct Game;
impl Game {
    pub const fn is_ck3()->bool { false }
    pub const fn is_vic3()->bool { false }
    pub const fn is_imperator()->bool { false }
    pub const fn is_eu5()->bool { false }
    pub const fn is_hoi4()->bool { false }
    pub const fn is_jomini()->bool { false }
}
