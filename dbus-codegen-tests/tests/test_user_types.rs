mod user_types_dbus;

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use dbus::blocking::Connection;
use dbus::channel::MatchingReceiver;
use dbus_crossroads::Crossroads;

use codegen_tests::user_type::MyType;
use user_types_dbus::*;

struct MyServer {
    t: MyType,
}

impl MyServer {
    fn new() -> Self {
        MyServer{t: MyType::new("123".to_string(), "456".to_string())}
    }
}

impl user_types_cr::ComExampleMyService1InterestingInterface for MyServer {
    fn method1(&mut self, arg1: codegen_tests::user_type::MyType) -> Result<codegen_tests::user_type::MyType, dbus::MethodErr>{
        return Ok(arg1);
    }
    fn bar(&self) -> Result<codegen_tests::user_type::MyType, dbus::MethodErr> {
        return Ok(self.t.clone());
    }
    fn set_bar(&self, _value: codegen_tests::user_type::MyType) -> Result<(), dbus::MethodErr> {
        return Ok(());
    }
}


#[test]
fn test_cr() {
    let c = Connection::new_session().unwrap();
    let cname = "com.example.dbustest";
    c.request_name(cname, false, true, false).unwrap();
    let mut cr = Crossroads::new();
    let token = user_types_cr::register_com_example_my_service1_interesting_interface::<MyServer>(&mut cr);
    let server = MyServer::new();
    cr.insert("/", &[token], server);
    c.start_receive(dbus::message::MatchRule::new_method_call(), Box::new(move |msg, conn| {
        cr.handle_message(msg, conn).unwrap();
        true
    }));

    let quit = std::sync::Arc::new(AtomicBool::new(false));
    let quit2 = quit.clone();

    let _ = std::thread::spawn(move || {
        {
            let c2 = dbus::blocking::Connection::new_session().unwrap();
            let p = c2.with_proxy(cname, "/", std::time::Duration::from_millis(1000));
            use user_types_blocking::ComExampleMyService1InterestingInterface;
            let arg = MyType::new("abc".to_string(), "cdf".to_string());
            assert_eq!(p.method1(&arg).unwrap(), arg);
        }

        let rt  = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let (resource, conn) = dbus_tokio::connection::new_session_sync().unwrap();
            let _handle = tokio::spawn(async {
                let err = resource.await;
                panic!("Lost connection to D-Bus: {}", err);
            });
            let p = dbus::nonblock::Proxy::new(cname, "/", Duration::from_secs(2), conn);
            use user_types_nonblock::ComExampleMyService1InterestingInterface;
            let arg = MyType::new("abc".to_string(), "cdf".to_string());
            assert_eq!(p.method1(&arg).await.unwrap(), arg);
        });

        quit2.store(true, Ordering::SeqCst);
    });

    loop { 
        c.process(std::time::Duration::from_millis(1000)).unwrap(); 
        if quit.load(Ordering::SeqCst) { break; }
    }
}
