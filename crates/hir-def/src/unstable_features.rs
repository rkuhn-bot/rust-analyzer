//! Handling of unstable features.
//!
//! We define two kinds of handling: we have a map of all unstable features for a crate
//! as `Symbol`s. This is mostly for external consumers.
//!
//! For analysis, we store them as a struct of bools, for fast access.

use std::fmt;

use base_db::{Crate, SourceDatabase};
use intern::{Symbol, sym};
use rustc_hash::FxHashSet;

impl fmt::Debug for UnstableFeatures {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(&self.all).finish()
    }
}

impl PartialEq for UnstableFeatures {
    fn eq(&self, other: &Self) -> bool {
        self.all == other.all
    }
}

impl Eq for UnstableFeatures {}

impl UnstableFeatures {
    #[inline]
    pub fn is_enabled(&self, feature: &Symbol) -> bool {
        self.all.contains(feature)
            || gca_feature_alias(feature).is_some_and(|alias| self.all.contains(&alias))
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = Symbol> {
        self.all.iter().cloned()
    }

    pub(crate) fn shrink_to_fit(&mut self) {
        self.all.shrink_to_fit();
    }
}

#[salsa::tracked]
impl UnstableFeatures {
    /// Query unstable features for a crate.
    ///
    /// This is also available as `DefMap::features()`. Use that if you have a DefMap available.
    /// Otherwise, use this, to not draw a dependency to the def map.
    #[salsa::tracked(returns(ref))]
    pub fn query(db: &dyn SourceDatabase, krate: Crate) -> UnstableFeatures {
        crate::crate_def_map(db, krate).features().clone()
    }
}

macro_rules! define_unstable_features {
    ( $( $feature:ident, )* ) => {
        #[derive(Clone, Default)]
        pub struct UnstableFeatures {
            all: FxHashSet<Symbol>,

            $( pub $feature: bool, )*
        }

        impl UnstableFeatures {
            pub(crate) fn enable(&mut self, feature: Symbol) {
                match () {
                    $( () if feature == sym::$feature => self.$feature = true, )*
                    _ => {}
                }

                if let Some(alias) = gca_feature_alias(&feature) {
                    self.all.insert(alias);
                }
                self.all.insert(feature);
            }
        }
    };
}

/// Either name in a pair from rust-lang/rust#163306 is the same gate.
fn gca_feature_alias(feature: &Symbol) -> Option<Symbol> {
    const PAIRS: &[(&Symbol, &Symbol)] = &[
        (&sym::min_generic_const_args, &sym::gca_min_const_items),
        (&sym::generic_const_args, &sym::gca_const_items),
        (&sym::macroless_generic_const_args, &sym::gca_macroless_args),
        (&sym::macroless_const_item_generic_const_args, &sym::gca_macroless_items),
    ];
    PAIRS.iter().find_map(|(old, new)| {
        if feature == *old {
            Some((*new).clone())
        } else if feature == *new {
            Some((*old).clone())
        } else {
            None
        }
    })
}

define_unstable_features! {
    lang_items,
    exhaustive_patterns,
    generic_associated_type_extended,
    arbitrary_self_types,
    arbitrary_self_types_pointers,
    supertrait_item_shadowing,
    new_range,
    never_type_fallback,
    specialization,
    min_specialization,
    ref_pat_eat_one_layer_2024,
    ref_pat_eat_one_layer_2024_structural,
    deref_patterns,
    mut_ref,
    type_changing_struct_update,
}

#[cfg(test)]
mod tests {
    use intern::sym;
    use test_fixture::WithFixture;

    use crate::{nameres::crate_def_map, test_db::TestDB};

    fn assert_alias(present: &str, present_sym: &intern::Symbol, other_sym: &intern::Symbol) {
        let fixture = format!("#![feature({present})]\n");
        let db = TestDB::with_files(&fixture);
        let krate = db.fetch_test_crate();
        let features = crate_def_map(&db, krate).features();
        assert!(features.is_enabled(present_sym), "{present} should enable itself");
        assert!(features.is_enabled(other_sym), "{present} should enable its alias");
    }

    #[test]
    fn generic_const_arg_feature_names_alias_each_other() {
        let pairs: &[(&str, &intern::Symbol, &str, &intern::Symbol)] = &[
            (
                "min_generic_const_args",
                &sym::min_generic_const_args,
                "gca_min_const_items",
                &sym::gca_min_const_items,
            ),
            (
                "generic_const_args",
                &sym::generic_const_args,
                "gca_const_items",
                &sym::gca_const_items,
            ),
            (
                "macroless_generic_const_args",
                &sym::macroless_generic_const_args,
                "gca_macroless_args",
                &sym::gca_macroless_args,
            ),
            (
                "macroless_const_item_generic_const_args",
                &sym::macroless_const_item_generic_const_args,
                "gca_macroless_items",
                &sym::gca_macroless_items,
            ),
        ];
        for (old, old_sym, new, new_sym) in pairs {
            assert_alias(old, old_sym, new_sym);
            assert_alias(new, new_sym, old_sym);
        }
    }
}
