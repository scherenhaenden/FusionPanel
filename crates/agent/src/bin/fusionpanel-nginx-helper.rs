#![forbid(unsafe_code)]

use agent::nginx_helper_rpc;

fn main() {
    let mut args = std::env::args_os().skip(1);
    let first = args.next();
    let second = args.next();
    let extra = args.next();
    let result = match (first, second, extra) {
        (None, None, None) => nginx_helper_rpc::run_helper_service(),
        (Some(flag), Some(path), None) if flag == "--socket" => {
            nginx_helper_rpc::run_helper_service_at(std::path::Path::new(&path))
        }
        _ => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "invalid helper arguments",
        )),
    };
    if result.is_err() {
        eprintln!("FusionPanel Nginx helper could not start its private RPC service");
        std::process::exit(1);
    }
}
