//! has_district has two scalar forms in the current triggers.log:739:
//! `yes` and a district key. The generic exported boolean flag does not make
//! district references invalid. 0.1.8 likewise maps this trigger to districts.
use crate::{ast::Node, engine::Engine};

impl Engine {
    /// Context-independent shape/reference checks, including unreachable code.
    /// Scope compatibility remains the normal rule.input check in triggers().
    pub(crate) fn district_trigger_structure(&mut self, n: &Node) -> bool {
        if n.key != "has_district" {
            return false;
        }
        if n.op.as_str() != "=" {
            self.gap(n, "has_district comparison operator semantics are not established by the current scalar examples", "district-comparison-unverified", None);
        }
        if self.scalar_field(n) && !matches!(n.scalar(), "yes" | "no") {
            // No numeric count/block grammar is documented for this trigger;
            // num_districts has its own separate quantity-query contract.
            self.symbol_reference("districts", n.scalar(), n);
        }
        true
    }
}
