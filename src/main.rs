extern crate tokio_core;
extern crate rustc_serialize;
extern crate getopts;
extern crate tokio_curl;
extern crate futures;
extern crate curl;
#[macro_use]
extern crate error_chain;

use std::env;
use std::io;
use std::time::Duration;

use futures::Future;
use futures::stream::{self, Stream};
use getopts::Options;
use tokio_core::reactor::{Core, Handle, Timeout};
use tokio_curl::Session;
use errors::*;

macro_rules! t {
    ($e:expr) => (match $e {
        Ok(e) => e,
        Err(e) => panic!("{} failed with {}", stringify!($e), e),
    })
}

type MyFuture<T> = Box<Future<Item=T, Error=BorsError>>;

#[derive(Clone)]
struct State {
    travis_token: String,
    appveyor_token: String,
    session: Session,
    repos: Vec<Repo>,
    branch: String,
    appveyor_account_name: String,
}

#[derive(Clone)]
struct Repo {
    user: String,
    name: String,
}

mod http;
mod errors;
mod travis;
mod appveyor;

fn main() {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let mut opts = Options::new();
    opts.reqopt("t", "travis", "travis token", "TOKEN");
    opts.reqopt("a", "appveyor", "appveyor token", "TOKEN");
    opts.reqopt("b", "branch", "branch to work with", "BRANCH");

    let usage = || -> ! {
        println!("{}", opts.usage("usage: ./foo -a ... -t ..."));
        std::process::exit(1);
    };

    let matches = match opts.parse(&args) {
        Ok(matches) => matches,
        Err(e) => {
            println!("error: {}", e);
            usage();
        }
    };

    let mut core = t!(Core::new());
    let handle = core.handle();

    let state = State {
        travis_token: matches.opt_str("t").unwrap(),
        appveyor_token: matches.opt_str("a").unwrap(),
        repos: matches.free.iter().map(|m| {
            let mut parts = m.splitn(2, '/');
            Repo {
                user: parts.next().unwrap().to_string(),
                name: parts.next().unwrap().to_string(),
            }
        }).collect(),
        session: Session::new(handle.clone()),
        branch: matches.opt_str("b").unwrap(),
        appveyor_account_name: "alexcrichton".to_string(),
    };

    let stream = stream::iter(std::iter::repeat(()).map(Ok::<_, BorsError>));
    core.run(stream.fold((), |(), ()| {
        state.check().and_then(|()| {
            t!(Timeout::new(Duration::new(10, 0), &handle))
                .map_err(|e| e.into())
        })
    })).unwrap();
}

impl State {
    fn check(&self) -> MyFuture<()> {
        let travis = self.check_travis();
        let appveyor = self.check_appveyor();
        Box::new(travis.join(appveyor).map(|_| ()))
    }

    fn check_travis(&self) -> MyFuture<()> {
        Box::new(futures::finished(()))
    }

    fn check_appveyor(&self) -> MyFuture<()> {
        let futures = self.repos.iter().map(|repo| {
            self.check_appveyor_repo(repo.clone())
        }).collect::<Vec<_>>();
        Box::new(futures::collect(futures).map(|_| ()))
    }

    fn check_appveyor_repo(&self, repo: Repo) -> MyFuture<()> {
        let url = format!("/projects/{}/{}/history?recordsNumber=10&branch={}",
                          self.appveyor_account_name,
                          repo.name,
                          self.branch);
        let history = http::appveyor_get(&self.session,
                                         &url,
                                         &self.appveyor_token);

        let me = self.clone();
        let repo2 = repo.clone();
        let cancel_old = history.and_then(move |history: appveyor::History| {
            let max = history.builds.iter().map(|b| b.buildNumber).max();
            let mut futures = Vec::new();
            for build in history.builds.iter() {
                if !me.appveyor_build_running(build) {
                    continue
                }
                if build.buildNumber < max.unwrap_or(0) {
                    println!("appveyor cancelling {} as it's not the latest",
                             build.buildNumber);
                    futures.push(me.appveyor_cancel_build(&repo2, build));
                }
            }
            futures::collect(futures)
        });

        let me = self.clone();
        let url = format!("/projects/{}/{}/branch/{}",
                          self.appveyor_account_name,
                          repo.name,
                          self.branch);
        let last_build = http::appveyor_get(&self.session,
                                            &url,
                                            &self.appveyor_token);
        let me = me.clone();
        let cancel_if_failed = last_build.and_then(move |last: appveyor::LastBuild| {
            if !me.appveyor_build_running(&last.build) {
                return Box::new(futures::finished(())) as Box<_>
            }
            for job in last.build.jobs.iter() {
                match &job.status[..] {
                    "success" |
                    "queued" |
                    "running" => continue,
                    _ => {}
                }

                println!("appveyor cancelling {} as a job is {}",
                         last.build.buildNumber,
                         job.status);
                return me.appveyor_cancel_build(&repo, &last.build)
            }
            Box::new(futures::finished(()))
        });

        Box::new(cancel_old.join(cancel_if_failed).map(|_| ()))
    }

    fn appveyor_build_running(&self, build: &appveyor::Build) -> bool {
        match &build.status[..] {
            "failed" |
            "cancelled" |
            "success" => false,
            _ => true,
        }
    }

    fn appveyor_cancel_build(&self, repo: &Repo, build: &appveyor::Build)
                             -> MyFuture<()> {
        let url = format!("/builds/{}/{}/{}",
                          self.appveyor_account_name,
                          repo.name,
                          build.version);
        http::appveyor_delete(&self.session, &url, &self.appveyor_token)
    }

}
