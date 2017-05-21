
use std::collections::hash_map::RandomState;
use std::collections::HashMap;
use std::hash::BuildHasher;
use std::hash::Hash;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::sync::mpsc::channel;
use std::borrow::Borrow;
use std::clone::Clone;
use std::sync::mpsc::TryRecvError;
use std::cell::RefCell;

use errors::ErrorKind::Disconnected;
use errors::Result;

pub fn keyvalue<K: Eq + Hash + Clone, V: Clone>
    ()
    -> (WritableHashMap<K, V>, ReadOnlyHashMap<K, V>)
{
    let (tx, rx) = channel();
    (WritableHashMap::new(tx), ReadOnlyHashMap::new(rx))
}

enum Action<K, V> {
    Add(K, V),
    Remove(K),
    Clear,
}

pub struct ReadOnlyHashMap<K, V, S = RandomState> {
    hashmap: RefCell<HashMap<K, V, S>>,
    rx: Receiver<Action<K, V>>,
}

impl<K, V> ReadOnlyHashMap<K, V, RandomState>
    where K: Eq + Hash + Clone,
          V: Clone
{
    fn new(rx: Receiver<Action<K, V>>) -> Self {
        ReadOnlyHashMap {
            hashmap: RefCell::new(HashMap::new()),
            rx: rx,
        }
    }
}

impl<K, V, S> ReadOnlyHashMap<K, V, S>
    where K: Eq + Hash + Clone,
          V: Clone,
          S: BuildHasher
{
    pub fn contains_key<Q: ?Sized>(&self, k: &Q) -> Result<bool>
        where K: Borrow<Q>,
              Q: Hash + Eq
    {
        self.process_changes()?;
        Ok(self.hashmap.borrow().contains_key(k))
    }

    pub fn get<Q: ?Sized>(&self, k: &Q) -> Result<Option<V>>
        where K: Borrow<Q>,
              Q: Hash + Eq
    {
        self.process_changes()?;
        Ok(self.hashmap.borrow().get(k).cloned())
    }

    fn process_changes(&self) -> Result<()> {
        loop {
            match self.rx.try_recv() {
                Ok(Action::Add(k, v)) => {
                    self.hashmap.borrow_mut().insert(k, v);
                }
                Ok(Action::Remove(k)) => {
                    self.hashmap.borrow_mut().remove(&k);
                }
                Ok(Action::Clear) => {
                    self.hashmap.borrow_mut().clear();
                }
                Err(TryRecvError::Empty) => return Ok(()),
                Err(TryRecvError::Disconnected) => return Err(Disconnected.into()),
            }
        }
    }
}


pub struct WritableHashMap<K, V, S = RandomState> {
    hashmap: HashMap<K, V, S>,
    tx: Sender<Action<K, V>>,
}


impl<K, V> WritableHashMap<K, V, RandomState>
    where K: Eq + Hash + Clone,
          V: Clone
{
    fn new(tx: Sender<Action<K, V>>) -> Self {
        WritableHashMap {
            hashmap: HashMap::new(),
            tx: tx,
        }
    }
}

impl<K, V, S> WritableHashMap<K, V, S>
    where K: Eq + Hash + Clone,
          V: Clone,
          S: BuildHasher
{
    pub fn clear(&mut self) -> Result<()> {
        self.hashmap.clear();
        self.tx.send(Action::Clear).map_err(|_| Disconnected.into())

    }

    pub fn get<Q: ?Sized>(&self, k: &Q) -> Option<&V>
        where K: Borrow<Q>,
              Q: Hash + Eq
    {
        self.hashmap.get(k)
    }

    pub fn contains_key<Q: ?Sized>(&self, k: &Q) -> bool
        where K: Borrow<Q>,
              Q: Hash + Eq
    {
        self.hashmap.contains_key(k)
    }

    pub fn insert(&mut self, k: K, v: V) -> Result<Option<V>> {
        self.tx
            .send(Action::Add(k.clone(), v.clone()))
            .map(|_| self.hashmap.insert(k, v))
            .map_err(|_| Disconnected.into())
    }

    //TODO:  It whould be good to loosen up this method signature so it matches
    //hashmap's remove.  I'm not sure exactly how to do that with the enum signature
    pub fn remove(&mut self, k: K) -> Result<Option<V>> {
        self.tx
            .send(Action::Remove(k.clone()))
            .map(|_| self.hashmap.remove(&k))
            .map_err(|_| Disconnected.into())
    }
}
