use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::time::Duration;
use std::time::Instant;

use crate::config::AegisConfig;

//TODO: we need to track IPs per route, that's the first one, otherwise we'll have to add regex support
//TODO: we also need to manage this so that we don't save every single IP that comes
//so, maybe that requires another data structure in which we discuss how to remove stale IPs
//i.e, if an IP has not made a request for n time, we remove it from our datastructure, and when it comes again, i.e. it's new
//we add it in and do the rate limiting for it

#[derive(Copy, Clone, Debug)]
pub enum StoreProcessResult {
    RateLimitExceeded,
    NotFound,
    Continue,
}

#[derive(Hash, Eq, PartialEq, PartialOrd, Clone, Debug)]
pub enum RequestIdentifier {
    Token(String),
    Ip(Ipv4Addr),
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
                tframe: now + Duration::from_secs(60),
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

    fn get_issue_time(&self) -> Instant {
        self.tframe - Duration::from_secs(60)
    }

    fn get_when_issued(&self) -> Duration {
        let now = Instant::now();
        let issue_time = self.get_issue_time();
        now - issue_time
    }

    fn is_stale(&self) -> bool {
        //for now, an IP is stale if it's been there for maybe 2 minutes
        Instant::now() >= (self.tframe + Duration::from_secs(120))
    }
}

#[derive(Debug)]
struct IpTable {
    store: HashMap<RequestIdentifier, DurationCount>,
    lim: usize,
}
impl IpTable {
    fn init_table(lim: usize) -> Self {
        IpTable {
            store: HashMap::new(),
            lim,
        }
    }

    fn check_or_add(&mut self, ri: RequestIdentifier) -> StoreProcessResult {
        let mut flag = StoreProcessResult::Continue;
        self.store
            .entry(ri)
            .and_modify(|v| flag = v.reduce())
            .or_insert_with(|| {
                let (v, s) = DurationCount::new(self.lim);
                flag = s;
                v
            });
        flag
    }

    //just like eager garbage collection, which seems to be the best strategy if we want average latencies
    fn gc(&mut self) {
        self.store.retain(|_, dc| !dc.is_stale());
    }
}

#[derive(Debug)]
pub struct InMemoryStore {
    backend_store: HashMap<String, IpTable>,
    routes_store: HashMap<String, IpTable>,
}

impl<'lt> InMemoryStore {
    pub fn init_empty(rcount: usize, bcount: usize) -> Self {
        let rstore = HashMap::with_capacity(rcount);
        let bstore = HashMap::with_capacity(bcount);
        return Self {
            backend_store: bstore,
            routes_store: rstore,
        };
    }

    pub fn fill(&mut self, config: &AegisConfig) {
        for bcfg in config.backend_config() {
            let url = if let Some(pfx) = &bcfg.prefix {
                let mut r_url = bcfg.url.clone();
                r_url.extend(pfx.chars());
                r_url
            } else {
                bcfg.url.clone()
            };
            let size = if let Some(v) = bcfg.rate_limit_ip_min {
                v
            } else {
                100
            };
            let ip_table = IpTable::init_table(size as usize);
            self.backend_store.insert(url, ip_table);
        }
        for rcfg in config.route_config() {
            let url = rcfg.url.clone();
            let size = if let Some(v) = rcfg.rate_limit_ip_min {
                v
            } else {
                100
            };
            let ip_table = IpTable::init_table(size as usize);
            self.routes_store.insert(url, ip_table);
        }
        return;
    }

    //lmao I'm legit willing to use lifetimes rather than clone
    //checks if we have the route, if so, yes, and we check if we've done the rate limiting, if so, yes
    pub fn process(&mut self, rpath: String, ip: RequestIdentifier) -> StoreProcessResult {
        //the flag will cascade if we haven't stored the route, i.e it's truly not found
        //so we have this -> rpath => routes? yes[return the result]: no[check the backend services? yes[return result]: no, then it's not found]
        let mut flag = StoreProcessResult::NotFound;
        self.routes_store.entry(rpath.clone()).and_modify(|v| {
            v.gc();
            flag = v.check_or_add(ip.clone())
        });
        match flag {
            StoreProcessResult::RateLimitExceeded | StoreProcessResult::Continue => return flag,
            _ => {}
        };
        self.backend_store.entry(rpath).and_modify(|v| {
            v.gc();
            flag = v.check_or_add(ip)
        });
        flag
    }
}
