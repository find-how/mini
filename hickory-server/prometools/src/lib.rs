#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod histogram;
pub mod nonstandard;
#[cfg(feature = "serde")]
#[cfg_attr(docsrs, doc(cfg(feature = "serde")))]
pub mod serde;
