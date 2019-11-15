use std::collections::{BTreeMap, HashMap};
use crate::message::MatchRule;
use crate::Message;
use crate::channel::Token;


pub type Replies<F> = HashMap<Token, F>;

pub struct Filters<F> {
    list: BTreeMap<Token, (MatchRule<'static>, F)>,
    nextid: Token,
}


impl<F> Default for Filters<F> {
    fn default() -> Self { Filters { list: BTreeMap::new(), nextid: Token(1), }}
}

impl<F> Filters<F> {
    pub fn add(&mut self, m: MatchRule<'static>, f: F) -> Token {
        let id = self.nextid;
        self.nextid.0 += 1;
        self.list.insert(id, (m, f));
        id
    }

    pub fn insert(&mut self, (t, m, f): (Token, MatchRule<'static>, F)) {
        self.list.insert(t, (m, f));
    }

    pub fn remove(&mut self, id: Token) -> Option<(MatchRule<'static>, F)> {
        self.list.remove(&id)
    }

    pub fn remove_matching(&mut self, msg: &Message) -> Option<(Token, MatchRule<'static>, F)> {
        if let Some(k) = self.list.iter_mut().find(|(_, v)| v.0.matches(&msg)).map(|(k, _)| *k) {
            let v = self.list.remove(&k).unwrap();
            Some((k, v.0, v.1))
        } else { None }
    }

}
