extern crate pkg_config;

fn main() {
    pkg_config::Config::new().atleast_version("1.6").probe("dbus-1").unwrap();
}
