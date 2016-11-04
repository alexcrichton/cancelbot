use std::str;
use std::io;

use tokio_curl;
use curl;
use rustc_serialize::json;

error_chain! {
    types {
        BorsError, BorsErrorKind, BorsChainErr, BorsResult;
    }

    foreign_links {
        curl::Error, Curl;
        tokio_curl::PerformError, TokioCurl;
        json::DecoderError, Json;
        str::Utf8Error, NotUtf8;
        io::Error, Io;
    }
}
