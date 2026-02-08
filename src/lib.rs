#![cfg_attr(not(test), no_std)]
extern crate alloc;

pub mod book;
pub(crate) mod central_directory;
pub mod errors;
pub(crate) mod nav;
pub(crate) mod ocf;
pub(crate) mod opf;
