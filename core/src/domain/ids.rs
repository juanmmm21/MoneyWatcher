use serde::{Deserialize, Serialize};

/// Identificadores tipados: evitan pasar por error el id de una cuenta donde se
/// espera el de una categoría, algo que un `i64` desnudo permitiría.
macro_rules! typed_id {
    ($name:ident) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub i64);

        impl $name {
            pub const fn value(self) -> i64 {
                self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl From<i64> for $name {
            fn from(value: i64) -> Self {
                Self(value)
            }
        }
    };
}

typed_id!(AccountId);
typed_id!(CategoryId);
typed_id!(TransactionId);
typed_id!(RuleId);
typed_id!(ImportId);
typed_id!(TransferLinkId);
