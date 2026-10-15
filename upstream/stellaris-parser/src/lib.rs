//! Tiger-derived syntax core for Stellaris. No game semantic tables are linked.
#[cfg(not(feature = "stellaris_parser"))]
compile_error!("The standalone parser requires its Stellaris lexical profile");
pub use token::{Loc,Token};
pub use report::take_reports;
mod block; mod date; mod token; mod pathtable; mod macros; mod tooltipped;
mod lowercase; mod helpers; mod fileset; mod game; mod parse; mod report;
pub mod stellaris_bridge;
