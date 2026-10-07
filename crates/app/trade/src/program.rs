//! The program: its name and types, which a view names it by.

use guest::Program;

use crate::{MODULE, Op, Query, Reply};

pub struct Trade;

impl Program for Trade {
    const NAME: &'static str = MODULE;
    type Op = Op;
    type Query = Query;
    type Reply = Reply;
}
