extern crate tokio_core;
extern crate getopts;
extern crate tokio_curl;
extern crate futures;

use std::env;

use getopts::Options;

fn main() {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let mut opts = Options::new();
    opts.reqopt("t", "travis", "travis token", "TOKEN");
    opts.reqopt("a", "appveyor", "appveyor token", "TOKEN");

    let matches = match opts.parse(&args) {
        Ok(matches) => matches,
        Err(e) => {
            println!("error: {}", e);
            println!("{}", opts.usage("usage: ./foo -a ... -t ..."));
            return
        }
    };

    let travis_token = matches.opt_str("t").unwrap();
    let appveyor_token = matches.opt_str("a").unwrap();

}
