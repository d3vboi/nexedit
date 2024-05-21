#![recursion_limit = "1024"]

#[macro_use]
extern crate error_chain;

#[macro_use]
extern crate lazy_static;

mod commands;
mod errors;
mod input;
mod models;
mod presenters;
mod util;
mod view;

pub use crate::errors::Error;
pub use crate::models::Application;
