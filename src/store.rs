use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::time::Duration;
use std::time::Instant;

//TODO: we need to track IPs per route, that's the first one, otherwise we'll have to add regex support
//TODO: we also need to manage this so that we don't save every single IP that comes
//so, maybe that requires another data structure in which we discuss how to remove stale IPs
//i.e, if an IP has not made a request for n time, we remove it from our datastructure, and when it comes again, i.e. it's new
//we add it in and do the rate limiting for it

#[derive(Debug)]
pub enum StoreProcessResult {
    RateLimitExceeded,
    NotFound,
    Continue,
}

#[derive(Debug)]
struct DurationCount {
    tframe: Instant,
    remain: usize,
    size: usize,
}

impl DurationCount {
    fn new(size: usize) -> (Self, StoreProcessResult) {
        let now = Instant::now();
        (
            Self {
                tframe: now,
                remain: size - 1,
                size,
            },
            StoreProcessResult::Continue,
        )
    }

    fn reduce(&mut self) -> StoreProcessResult {
        self.check_or_refresh();
        if self.remain <= 0 {
            return StoreProcessResult::RateLimitExceeded;
        } else {
            self.remain -= 1;
            return StoreProcessResult::Continue;
        }
    }

    fn check_or_refresh(&mut self) {
        let now = Instant::now();
        if self.tframe >= now {
            return;
        } else {
            self.tframe = now + Duration::from_secs(60);
            self.remain = self.size;
        }
    }
}

#[derive(Debug)]
struct IpTable {
    store: HashMap<Ipv4Addr, DurationCount>,
    lim: usize,
}
impl IpTable {
    fn init_table(lim: usize) -> Self {
        IpTable {
            store: HashMap::new(),
            lim,
        }
    }

    fn check_or_add(&mut self, ip: Ipv4Addr) -> StoreProcessResult {
        let mut flag = StoreProcessResult::Continue;
        self.store
            .entry(ip)
            .and_modify(|v| flag = v.reduce())
            .or_insert_with(|| {
                let (v, s) = DurationCount::new(self.lim);
                flag = s;
                v
            });
        flag
    }
}

#[derive(Debug)]
pub struct InMemoryStore<'lt> {
    backend_store: HashMap<&'lt str, IpTable>,
    routes_store: HashMap<&'lt str, IpTable>,
}

impl<'lt> InMemoryStore<'lt> {
    pub fn init_empty(rcount: usize, bcount: usize) -> Self {
        let rstore = HashMap::with_capacity(rcount);
        let bstore = HashMap::with_capacity(bcount);
        return Self {
            backend_store: bstore,
            routes_store: rstore,
        };
    }

    //lmao I'm legit willing to use lifetimes rather than clone
    //checks if we have the route, if so, yes, and we check if we've done the rate limiting, if so, yes
    pub fn process(&mut self, rpath: &'lt str, ip: Ipv4Addr) -> StoreProcessResult {
        let mut flag = StoreProcessResult::NotFound;
        self.routes_store
            .entry(rpath)
            .and_modify(|v| flag = v.check_or_add(ip));
        match flag {
            StoreProcessResult::RateLimitExceeded | StoreProcessResult::Continue => return flag,
            _ => {}
        };
        self.backend_store
            .entry(rpath)
            .and_modify(|v| flag = v.check_or_add(ip));
        flag
    }
}
