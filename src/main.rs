extern crate domain;
extern crate tokio_core;
extern crate futures;

use std::net::IpAddr;
use std::thread;

use futures::{Stream, Future};
use futures::sync::{oneshot, mpsc as fmpsc};
use std::sync::mpsc;
use tokio_core::reactor::Core;
use domain::resolv::Resolver;
use domain::resolv::lookup::addr::lookup_addr;

fn main() {
    let ip_addr = "172.217.3.238".parse::<IpAddr>().unwrap();
    
    // Use Naive Solution
    let hostnames = lookup_hostnames(ip_addr);
    println!("[NAIVE] Reverse Ip Look Results For: {}", ip_addr);
    for hostname in hostnames {
        println!(" - {}", hostname);
    }

    // Blog Post Solution
    println!("[BLOG] Reverse Ip Look Results For: {}", ip_addr);
    let handle = create_lookup_handle();
    let result_future = handle.lookup_hostnames(ip_addr);
    for hostname in result_future.wait().unwrap() {
        println!(" - {}", hostname);
    }

    // Improved Simplified Solution
    println!("[Simple] Reverse Ip Look Results For: {}", ip_addr);
    let handle = create_simple_lookup_handle();
    let result_future = handle.lookup_hostnames(ip_addr);
    for hostname in result_future.wait().unwrap() {
        println!(" - {}", hostname);
    }

}

//
//  Naive Solution
//

fn lookup_hostnames(ip: IpAddr) -> Vec<String> {
    let mut core = Core::new().unwrap();
    let resolv = Resolver::new(&core.handle());

    let addrs = lookup_addr(resolv, ip);
    let names_response = core.run(addrs).unwrap();
    names_response.iter().map(|n| n.to_string()).collect()
}


//
//  Actual Solution
//

fn create_lookup_handle() -> DnsLookupHandle {
    let (req_tx, req_rx) = fmpsc::unbounded::<ReverseLookupRequest>();

    thread::spawn(move || {
        let mut core = Core::new().unwrap();
        let core_handle = core.handle();
        let resolv = Resolver::new(&core.handle()); 
        let resolver_loop =
            req_rx.map_err(|e| println!("error = {:?}", e))
                  .for_each(move |request| {
                      let future = handle_reverse_lookup(request, resolv.clone());
                      core_handle.spawn(future);
                      Ok(())
                  });

        core.run(resolver_loop).expect("[dns] Failed to start reactor core loop.");
    });

    DnsLookupHandle { request_sender: req_tx }
}

fn handle_reverse_lookup(request: ReverseLookupRequest, resolv: Resolver) -> impl Future<Item=(), Error=()> {
    lookup_addr(resolv, request.ip).then(|result| {
        let response = 
            match result {
                Ok(addrs) => ReverseLookupResponse { names: addrs.iter().map(|n| n.to_string()).collect() },
                Err(_) => ReverseLookupResponse { names: Vec::new() },
            };
        request.sender.send(response);
        Ok(())
    })
}

struct ReverseLookupRequest {
    ip: IpAddr,
    sender: oneshot::Sender<ReverseLookupResponse>
}

struct ReverseLookupResponse {
    names: Vec<String>
}
 
#[derive(Clone)]
pub struct DnsLookupHandle {
    request_sender: fmpsc::UnboundedSender<ReverseLookupRequest>,
}

impl DnsLookupHandle {
    pub fn lookup_hostnames(&self, ip: IpAddr) -> impl Future<Item=Vec<String>, Error=oneshot::Canceled> {
        let (resp_tx, resp_rx) = oneshot::channel::<ReverseLookupResponse>();
        let result = self.request_sender.unbounded_send(ReverseLookupRequest { ip: ip, sender: resp_tx });
        resp_rx.map(|res| res.names)
    }
}

//
// Simplified Solution
//

fn create_simple_lookup_handle() -> SimpleDnsLookupHandle {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut core = Core::new().unwrap();
        let resolv = Resolver::new(&core.handle()); 
        tx.send(resolv);
        loop { core.turn(None); }
    });

    return SimpleDnsLookupHandle { resolv: rx.recv().unwrap() }
}


#[derive(Clone)]
pub struct SimpleDnsLookupHandle {
    resolv: Resolver,
}

impl SimpleDnsLookupHandle {
    pub fn lookup_hostnames(&self, ip: IpAddr) -> impl Future<Item=Vec<String>, Error=()> {
        lookup_addr(self.resolv.clone(), ip).map_err(|e| println!("error = {:?}", e))
                                    .map(|addrs| addrs.iter().map(|n| n.to_string()).collect())
    }
}
